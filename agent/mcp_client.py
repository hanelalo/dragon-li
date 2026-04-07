import asyncio
import logging
from typing import Dict, Any, List, Optional
from contextlib import AsyncExitStack

from mcp.client.session import ClientSession
from mcp.client.stdio import stdio_client
from mcp.client.stdio import StdioServerParameters

logger = logging.getLogger("uvicorn.error")

class McpClientManager:
    def __init__(self):
        self.sessions: Dict[str, ClientSession] = {}
        self.exit_stack = AsyncExitStack()
        self._tools_cache: Dict[str, List[Any]] = {}

    async def connect(self, name: str, params: StdioServerParameters):
        try:
            stdio_ctx = stdio_client(params)
            read_stream, write_stream = await self.exit_stack.enter_async_context(stdio_ctx)
            
            session = ClientSession(read_stream, write_stream)
            await self.exit_stack.enter_async_context(session)
            
            await session.initialize()
            
            self.sessions[name] = session
            logger.info(f"Connected to MCP server: {name}")
            
            # Fetch tools right after connection to cache them
            tools_response = await session.list_tools()
            self._tools_cache[name] = tools_response.tools
            logger.info(f"Loaded {len(tools_response.tools)} tools from MCP server {name}")
            
        except Exception as e:
            logger.error(f"Failed to connect to MCP server {name}: {e}")
            raise

    def get_all_tools(self) -> List[Dict[str, Any]]:
        all_tools = []
        for name, tools in self._tools_cache.items():
            for tool in tools:
                # Add server prefix to tool names to avoid conflicts
                tool_dict = {
                    "type": "function",
                    "function": {
                        "name": f"{name}__{tool.name}",
                        "description": tool.description or "",
                        "parameters": tool.inputSchema
                    }
                }
                all_tools.append(tool_dict)
        return all_tools

    async def call_tool(self, name: str, arguments: dict) -> Any:
        # name format is server_name__tool_name
        if "__" not in name:
            raise ValueError(f"Invalid tool name format: {name}")
            
        server_name, tool_name = name.split("__", 1)
        if server_name not in self.sessions:
            raise ValueError(f"MCP server {server_name} not found")
            
        session = self.sessions[server_name]
        result = await session.call_tool(tool_name, arguments)
        return result

    async def close(self):
        await self.exit_stack.aclose()

# Global singleton
mcp_manager = McpClientManager()
