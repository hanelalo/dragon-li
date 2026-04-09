import json
import logging
from typing import Dict, Any, List

from .builtin import execute_web_search, fetch_webpage, get_current_time
from agent_mcp.client import mcp_manager

logger = logging.getLogger("uvicorn.error")

def get_tools_for_openai(enable_web_search: bool, cfg: Any) -> List[Dict[str, Any]]:
    tools = mcp_manager.get_all_tools()
    
    # Inject get_current_time tool
    tools.append({
        "type": "function",
        "function": {
            "name": "get_current_time",
            "description": "Get the exact current system time.",
            "parameters": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }
    })

    # Inject fetch_webpage tool
    tools.append({
        "type": "function",
        "function": {
            "name": "fetch_webpage",
            "description": "Fetch the specified URL and extract its core text content. Use this to read documentation, articles, and websites.",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The full URL to fetch, e.g., 'https://example.com/doc'"
                    }
                },
                "required": ["url"]
            }
        }
    })

    if enable_web_search and cfg and cfg.tools and cfg.tools.brave_search_api_key:
        tools.append({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web for real-time information.",
                "parameters": {
                    "type": "object",
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"]
                }
            }
        })
    return tools

def get_tools_for_anthropic(enable_web_search: bool, cfg: Any) -> List[Dict[str, Any]]:
    mcp_tools = mcp_manager.get_all_tools()
    anthropic_tools = []
    
    if mcp_tools:
        for t in mcp_tools:
            anthropic_tools.append({
                "name": t["function"]["name"],
                "description": t["function"]["description"],
                "input_schema": t["function"]["parameters"]
            })
    
    # Inject get_current_time tool
    anthropic_tools.append({
        "name": "get_current_time",
        "description": "Get the exact current system time.",
        "input_schema": {
            "type": "object",
            "properties": {},
            "required": []
        }
    })

    # Inject fetch_webpage tool
    anthropic_tools.append({
        "name": "fetch_webpage",
        "description": "Fetch the specified URL and extract its core text content. Use this to read documentation, articles, and websites.",
        "input_schema": {
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The full URL to fetch, e.g., 'https://example.com/doc'"
                }
            },
            "required": ["url"]
        }
    })

    if enable_web_search and cfg and cfg.tools and cfg.tools.brave_search_api_key:
        anthropic_tools.append({
            "name": "web_search",
            "description": "Search the web for real-time information.",
            "input_schema": {
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"]
            }
        })
        
    return anthropic_tools

async def execute_tool(name: str, arguments_str: str, req_cfg: Any, session_id: str) -> str:
    logger.info(f"[Session: {session_id}] Executing tool: {name} | Args: {arguments_str}")
    try:
        args = json.loads(arguments_str) if arguments_str else {}
        if name == "get_current_time":
            result_str = get_current_time()
        elif name == "fetch_webpage":
            result_str = await fetch_webpage(args.get("url", ""))
        elif name == "web_search":
            api_key = req_cfg.tools.brave_search_api_key if (req_cfg and req_cfg.tools and req_cfg.tools.brave_search_api_key) else ""
            result_str = await execute_web_search(args.get("query", ""), api_key)
        else:
            result = await mcp_manager.call_tool(name, args)
            if hasattr(result, "content") and isinstance(result.content, list):
                texts = [c.text for c in result.content if hasattr(c, "text")]
                result_str = "\n".join(texts)
            else:
                result_str = str(result)
    except Exception as e:
        logger.error(f"[Session: {session_id}] Tool {name} failed: {e}")
        result_str = f"Error: {e}"
        
    logger.info(f"[Session: {session_id}] Tool {name} returned {len(result_str)} chars.")
    return result_str
