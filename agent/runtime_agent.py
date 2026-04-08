#!/usr/bin/env python3
"""Minimal runtime agent for desktop-runtime-core bootstrap."""

import time
_START_TIME = time.time()

import argparse
import json
import sys
import sqlite3

from fastapi import FastAPI, HTTPException
from fastapi.responses import StreamingResponse
import uvicorn
import logging
from contextlib import asynccontextmanager
import os
import shlex
import asyncio

from models import ChatRequestInput, TitleGenerateRequest, TitleGenerateResponse, MemoryExtractRequest, MemoryExtractResponse
from llm_provider import chat_stream_generator, generate_title, extract_memories
from mcp_client import mcp_manager
from mcp.client.stdio import StdioServerParameters

logger = logging.getLogger("uvicorn.error")

async def load_mcp_from_db():
    db_path = os.environ.get("DRAGON_LI_DB_PATH")
    if not db_path or not os.path.exists(db_path):
        logger.warning("DRAGON_LI_DB_PATH not set or not found, skipping dynamic MCP connectors")
        return

    try:
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row
        cursor = conn.cursor()
        try:
            cursor.execute("SELECT name, mcp_type, config_content FROM mcp_connectors WHERE deleted_at IS NULL")
            connectors = cursor.fetchall()
            tasks = []
            for row in connectors:
                name = row["name"]
                mcp_type = row["mcp_type"]
                try:
                    config = json.loads(row["config_content"])
                except Exception as e:
                    logger.warning(f"Failed to parse config for {name}: {e}")
                    continue
                
                if not config.get("enabled", False):
                    continue

                if mcp_type == "stdio":
                    command = config.get("command")
                    args = config.get("args", [])
                    env = config.get("env", {})
                    if not command:
                        logger.warning(f"MCP {name} is missing command")
                        continue
                    
                    # Merge with current env
                    full_env = os.environ.copy()
                    full_env.update(env)

                    logger.info(f"Connecting to stdio MCP server: {name} via {command}")
                    tasks.append(mcp_manager.connect_stdio(
                        name,
                        StdioServerParameters(
                            command=command,
                            args=args,
                            env=full_env
                        )
                    ))
                elif mcp_type == "sse":
                    url = config.get("url")
                    headers = config.get("headers", {})
                    if not url:
                        logger.warning(f"MCP {name} is missing url")
                        continue
                    logger.info(f"Connecting to sse MCP server: {name} at {url}")
                    tasks.append(mcp_manager.connect_sse(name, url, headers))
                elif mcp_type == "streamable_http":
                    url = config.get("url")
                    headers = config.get("headers", {})
                    if not url:
                        logger.warning(f"MCP {name} is missing url")
                        continue
                    logger.info(f"Connecting to streamable_http MCP server: {name} at {url}")
                    tasks.append(mcp_manager.connect_streamable_http(name, url, headers))
            
            if tasks:
                results = await asyncio.gather(*tasks, return_exceptions=True)
                for result in results:
                    if isinstance(result, Exception):
                        logger.error(f"Error during concurrent MCP connection: {result}")

        except sqlite3.OperationalError as e:
            logger.warning(f"Failed to read mcp_connectors table: {e}")
        finally:
            conn.close()
    except Exception as e:
        logger.warning(f"Failed to connect to MCP servers from DB: {e}")

@asynccontextmanager
async def lifespan(app: FastAPI):
    # 将 MCP 加载放到后台任务中，避免阻塞服务启动
    asyncio.create_task(load_mcp_from_db())
    logger.info(f"====== Python Agent Started in {time.time() - _START_TIME:.3f} seconds ======")
    yield
    await mcp_manager.close()

app = FastAPI(title="Dragon-Li Runtime Agent", lifespan=lifespan)

from pydantic import BaseModel
from mcp.client.session import ClientSession
from mcp.client.stdio import stdio_client

from mcp.client.sse import sse_client
from mcp.client.streamable_http import streamable_http_client

class McpTestRequest(BaseModel):
    mcp_type: str
    config_content: str

@app.post("/v1/mcp/reload")
async def mcp_reload():
    logger.info("Reloading MCP connections...")
    await mcp_manager.close()
    await load_mcp_from_db()
    return {"status": "ok"}

@app.get("/v1/mcp/status")
async def mcp_status():
    status = {}
    for name, session in mcp_manager.sessions.items():
        tools = mcp_manager._tools_cache.get(name, [])
        tool_list = [{"name": t.name, "description": getattr(t, "description", "")} for t in tools]
        status[name] = {
            "status": "connected",
            "tools": tool_list,
        }
    return status

@app.post("/v1/mcp/test")
async def mcp_test(req: McpTestRequest):
    try:
        config = json.loads(req.config_content)
    except Exception as e:
        return {"status": "error", "error": f"Invalid JSON config: {e}"}
    
    if req.mcp_type == "stdio":
        command = config.get("command")
        args = config.get("args", [])
        env = config.get("env", {})
        
        if not command:
            return {"status": "error", "error": "Missing command for stdio"}
        
        full_env = os.environ.copy()
        full_env.update(env)
        
        try:
            params = StdioServerParameters(command=command, args=args, env=full_env)
            async with stdio_client(params) as (read, write):
                async with ClientSession(read, write) as session:
                    await session.initialize()
                    tools = await session.list_tools()
            tool_list = [{"name": t.name, "description": getattr(t, "description", "")} for t in tools.tools]
            return {"status": "ok", "tools_count": len(tools.tools), "tools": tool_list}
        except Exception as e:
            return {"status": "error", "error": str(e)}
    elif req.mcp_type == "sse":
        url = config.get("url")
        headers = config.get("headers", {})
        if not url:
            return {"status": "error", "error": "Missing url for sse"}
        try:
            async with sse_client(url, headers=headers) as (read, write):
                async with ClientSession(read, write) as session:
                    await session.initialize()
                    tools = await session.list_tools()
            tool_list = [{"name": t.name, "description": getattr(t, "description", "")} for t in tools.tools]
            return {"status": "ok", "tools_count": len(tools.tools), "tools": tool_list}
        except Exception as e:
            return {"status": "error", "error": str(e)}
            
    elif req.mcp_type == "streamable_http":
        url = config.get("url")
        headers = config.get("headers", {})
        if not url:
            return {"status": "error", "error": "Missing url for streamable_http"}
        try:
            import httpx
            client = httpx.AsyncClient(headers=headers) if headers else None
            async with streamable_http_client(url, http_client=client) as streams:
                read, write = streams[0], streams[1]
                async with ClientSession(read, write) as session:
                    await session.initialize()
                    tools = await session.list_tools()
            tool_list = [{"name": t.name, "description": getattr(t, "description", "")} for t in tools.tools]
            return {"status": "ok", "tools_count": len(tools.tools), "tools": tool_list}
        except Exception as e:
            return {"status": "error", "error": str(e)}
    else:
        return {"status": "error", "error": f"Unknown mcp_type: {req.mcp_type}"}


@app.get("/health")
def health_check():
    return {"status": "ok"}

@app.post("/v1/chat/stream")
async def chat_stream(req: ChatRequestInput):
    return StreamingResponse(
        chat_stream_generator(req),
        media_type="text/event-stream",
    )

@app.post("/v1/chat/summarize_title")
async def summarize_title(req: TitleGenerateRequest) -> TitleGenerateResponse:
    try:
        title = await generate_title(req)
        title = title.strip().strip('"\'「」\n')
        if len(title) > 30:
            title = title[:27] + "..."
        if not title:
            title = "New Chat"
        return TitleGenerateResponse(title=title)
    except Exception as e:
        logger.exception("Failed to summarize title")
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/v1/memory/extract")
async def memory_extract(req: MemoryExtractRequest) -> MemoryExtractResponse:
    try:
        data = await extract_memories(req)
        memories = data.get("memories", [])
        return MemoryExtractResponse(memories=memories)
    except Exception as e:
        logger.exception("Failed to extract memory")
        raise HTTPException(status_code=500, detail=str(e))

def run_server(uds_path: str) -> int:
    if not uds_path:
        print(json.dumps({"ok": False, "error": "--uds is required for --serve"}))
        return 1
    
    # Run uvicorn with the unix domain socket
    uvicorn.run(app, uds=uds_path)
    return 0

def main() -> int:
    parser = argparse.ArgumentParser(description="Dragon-Li runtime agent")
    parser.add_argument("--serve", action="store_true")
    parser.add_argument("--uds", type=str, help="Path to the Unix Domain Socket")
    parser.add_argument("--db-path", type=str, help="Path to the SQLite database")
    args = parser.parse_args()
    
    if args.db_path:
        os.environ["DRAGON_LI_DB_PATH"] = args.db_path

    if args.serve:
        return run_server(args.uds)

    print(json.dumps({"ok": False, "error": "No mode selected"}))
    return 1

if __name__ == "__main__":
    raise SystemExit(main())
