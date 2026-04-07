#!/usr/bin/env python3
"""Minimal runtime agent for desktop-runtime-core bootstrap."""

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

from models import ChatRequestInput, TitleGenerateRequest, TitleGenerateResponse, MemoryExtractRequest, MemoryExtractResponse
from llm_provider import chat_stream_generator, generate_title, extract_memories
from mcp_client import mcp_manager
from mcp.client.stdio import StdioServerParameters

logger = logging.getLogger("uvicorn.error")

@asynccontextmanager
async def lifespan(app: FastAPI):
    db_path = os.environ.get("DRAGON_LI_DB_PATH")
    if db_path and os.path.exists(db_path):
        try:
            conn = sqlite3.connect(db_path)
            conn.row_factory = sqlite3.Row
            cursor = conn.cursor()
            # Find all enabled MCP connectors
            # Handle case where table might not exist yet
            try:
                cursor.execute("SELECT name, mcp_type, endpoint FROM mcp_connectors WHERE enabled = 1 AND deleted_at IS NULL")
                connectors = cursor.fetchall()
                for row in connectors:
                    name = row["name"]
                    mcp_type = row["mcp_type"]
                    endpoint = row["endpoint"]
                    logger.info(f"Connecting to MCP server: {name} (type: {mcp_type}) at {endpoint}")
                    
                    if mcp_type == 'stdio':
                        # Parse endpoint command into args
                        args = shlex.split(endpoint)
                        if not args:
                            continue
                        command = args[0]
                        command_args = args[1:]
                        await mcp_manager.connect(
                            name,
                            StdioServerParameters(
                                command=command,
                                args=command_args
                            )
                        )
                    else:
                        logger.warning(f"MCP type '{mcp_type}' is not yet supported by runtime agent.")
            except sqlite3.OperationalError as e:
                logger.warning(f"Failed to read mcp_connectors table: {e}")
            finally:
                conn.close()
        except Exception as e:
            logger.warning(f"Failed to connect to MCP servers from DB: {e}")
    else:
        logger.warning("DRAGON_LI_DB_PATH not set or not found, skipping dynamic MCP connectors")
    
    yield
    
    await mcp_manager.close()

app = FastAPI(title="Dragon-Li Runtime Agent", lifespan=lifespan)


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
