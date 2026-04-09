import json
import logging
from typing import Dict, Any, List

from .builtin import execute_web_search, fetch_webpage, get_current_time
from agent_mcp.client import mcp_manager

logger = logging.getLogger("uvicorn.error")

def get_tools_for_openai(enable_web_search: bool, cfg: Any, explicit_skill_id: str = None) -> List[Dict[str, Any]]:
    allowed_tools = None
    if explicit_skill_id:
        from skills.manager import skill_manager
        tools = skill_manager.get_tools_for_skill(explicit_skill_id)
        allowed_tools = skill_manager.get_allowed_tools(explicit_skill_id)
    else:
        tools = mcp_manager.get_all_tools()
        
    def is_allowed(name: str) -> bool:
        if allowed_tools is None:
            return True
        return name in allowed_tools
    
    # Inject get_current_time tool
    if is_allowed("get_current_time"):
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
    if is_allowed("fetch_webpage"):
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
        if is_allowed("web_search"):
            # Check if web_search is already in tools (from MCP) to avoid duplicates
            if not any(t.get("function", {}).get("name") == "web_search" for t in tools):
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

def get_tools_for_anthropic(enable_web_search: bool, cfg: Any, explicit_skill_id: str = None) -> List[Dict[str, Any]]:
    allowed_tools = None
    if explicit_skill_id:
        from skills.manager import skill_manager
        mcp_tools = skill_manager.get_tools_for_skill(explicit_skill_id)
        allowed_tools = skill_manager.get_allowed_tools(explicit_skill_id)
    else:
        mcp_tools = mcp_manager.get_all_tools()
        
    anthropic_tools = []
    
    if mcp_tools:
        for t in mcp_tools:
            anthropic_tools.append({
                "name": t["function"]["name"],
                "description": t["function"]["description"],
                "input_schema": t["function"]["parameters"]
            })
            
    def is_allowed(name: str) -> bool:
        if allowed_tools is None:
            return True
        return name in allowed_tools
    
    # Inject get_current_time tool
    if is_allowed("get_current_time"):
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
    if is_allowed("fetch_webpage"):
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
        if is_allowed("web_search"):
            if not any(t.get("name") == "web_search" for t in anthropic_tools):
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

async def execute_tool(name: str, arguments_str: str, req_cfg: Any, session_id: str, explicit_skill_id: str = None) -> str:
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
            is_local = False
            if explicit_skill_id:
                from skills.manager import skill_manager
                try:
                    result_str = skill_manager.execute_local_tool(explicit_skill_id, name, args)
                    is_local = True
                except Exception as e:
                    if "not found in skill" in str(e):
                        is_local = False
                    else:
                        raise e
            
            if not is_local:
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
