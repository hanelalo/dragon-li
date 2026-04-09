import os
import json
import logging
from pydantic import BaseModel
from fastapi import APIRouter

from mcp.client.session import ClientSession
from mcp.client.stdio import stdio_client, StdioServerParameters
from mcp.client.sse import sse_client
from mcp.client.streamable_http import streamable_http_client

from agent_mcp.client import mcp_manager
import sqlite3
import asyncio

logger = logging.getLogger("uvicorn.error")

router = APIRouter()

class McpTestRequest(BaseModel):
    mcp_type: str
    config_content: str

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

@router.post("/reload")
async def mcp_reload():
    logger.info("Reloading MCP connections...")
    await mcp_manager.close()
    await load_mcp_from_db()
    return {"status": "ok"}

@router.get("/status")
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

@router.post("/test")
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