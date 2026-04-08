import logging
from typing import AsyncGenerator
from openai import AsyncOpenAI
from anthropic import AsyncAnthropic

import json
from models import (
    ChatRequestInput,
    ChatStreamEvent,
    ChatStreamEventDelta,
    ChatStreamEventReasoning,
    ChatStreamEventUsage,
    ChatStreamEventDone,
    ChatStreamEventAborted,
    event_to_json,
    ApiProfile,
    TitleGenerateRequest,
    MemoryExtractRequest
)
from prompts import TITLE_GENERATION_PROMPT, MEMORY_EXTRACTION_PROMPT, CHAT_SYSTEM_PROMPT_TEMPLATE

from mcp_client import mcp_manager

logger = logging.getLogger("uvicorn.error")

async def chat_stream_generator(req: ChatRequestInput) -> AsyncGenerator[str, None]:
    """Generates SSE lines from the LLM provider."""
    try:
        # Find the profile from the injected config
        profile = None
        if req.cfg and req.cfg.profiles:
            profile = next((p for p in req.cfg.profiles if p.id == req.profile_id), None)
        
        if not profile:
            yield f"data: {event_to_json(ChatStreamEventAborted(code='CONFIG_PROFILE_NOT_FOUND', message=f'Profile not found: {req.profile_id}', retryable=False))}\n\n"
            return
        
        if not profile.enabled:
            yield f"data: {event_to_json(ChatStreamEventAborted(code='CONFIG_PROFILE_NOT_FOUND', message=f'Profile disabled: {req.profile_id}', retryable=False))}\n\n"
            return

        model = req.model or profile.default_model

        if profile.provider == "openai":
            async for event in _openai_stream(req, profile, model):
                yield f"data: {event_to_json(event)}\n\n"
        elif profile.provider == "anthropic":
            async for event in _anthropic_stream(req, profile, model):
                yield f"data: {event_to_json(event)}\n\n"
        else:
            yield f"data: {event_to_json(ChatStreamEventAborted(code='INVALID_REQUEST', message=f'Unknown provider: {profile.provider}', retryable=False))}\n\n"
            return

    except Exception as e:
        logger.exception("LLM Stream failed")
        yield f"data: {event_to_json(ChatStreamEventAborted(code='PROVIDER_SERVER_ERROR', message=str(e), retryable=True))}\n\n"

def _build_chat_system_content(req: ChatRequestInput) -> str:
    memory_part = req.prompt.memory.strip()
    memory_section = f"# Context (Injected Memories)\n{memory_part}\n\n" if memory_part else ""
    
    base_system = CHAT_SYSTEM_PROMPT_TEMPLATE.format(memory_section=memory_section)
    system_parts = [base_system, req.prompt.system, req.prompt.runtime]
    
    return "\n\n".join(p for p in system_parts if p.strip())

async def _openai_stream(req: ChatRequestInput, profile: ApiProfile, model: str) -> AsyncGenerator[ChatStreamEvent, None]:
    client = AsyncOpenAI(api_key=profile.api_key, base_url=profile.base_url)
    
    messages = []
    
    system_content = _build_chat_system_content(req)
    if system_content:
        messages.append({"role": "system", "content": system_content})
    
    for msg in req.history:
        messages.append({"role": msg.role, "content": msg.content})
    
    user_content = req.prompt.user.strip()
    if user_content:
        messages.append({"role": "user", "content": user_content})
    else:
        messages.append({"role": "user", "content": " "})
    
    while True:
        try:
            kwargs = {
                "model": model,
                "messages": messages,
                "stream": True,
                "stream_options": {"include_usage": True},
            }
            tools = mcp_manager.get_all_tools()
            if tools:
                kwargs["tools"] = tools

            response = await client.chat.completions.create(**kwargs)
            
            tool_calls_accumulator = {}
            accumulated_content = ""
            
            async for chunk in response:
                if chunk.usage:
                    if hasattr(chunk.usage, 'prompt_tokens') and hasattr(chunk.usage, 'completion_tokens'):
                        yield ChatStreamEventUsage(
                            tokens_in=chunk.usage.prompt_tokens,
                            tokens_out=chunk.usage.completion_tokens,
                        )
                if chunk.choices and len(chunk.choices) > 0:
                    delta = chunk.choices[0].delta
                    if hasattr(delta, "reasoning_content") and delta.reasoning_content:
                        yield ChatStreamEventReasoning(text=delta.reasoning_content)
                    if delta.content:
                        accumulated_content += delta.content
                        yield ChatStreamEventDelta(text=delta.content)
                    if getattr(delta, "tool_calls", None):
                        for tc in delta.tool_calls:
                            if tc.index not in tool_calls_accumulator:
                                tool_calls_accumulator[tc.index] = {
                                    "id": tc.id,
                                    "type": "function",
                                    "function": {"name": tc.function.name or "", "arguments": tc.function.arguments or ""}
                                }
                            else:
                                if tc.function.name:
                                    tool_calls_accumulator[tc.index]["function"]["name"] += tc.function.name
                                if tc.function.arguments:
                                    tool_calls_accumulator[tc.index]["function"]["arguments"] += tc.function.arguments
                            
            if not tool_calls_accumulator:
                yield ChatStreamEventDone()
                break
                
            # Execute tool calls
            messages.append({
                "role": "assistant",
                "content": accumulated_content or None,
                "tool_calls": list(tool_calls_accumulator.values())
            })
            
            for tc in tool_calls_accumulator.values():
                name = tc["function"]["name"]
                arguments_str = tc["function"]["arguments"]
                logger.info(f"[Session: {req.session_id}] Executing MCP tool: {name} | Args: {arguments_str}")
                
                try:
                    args = json.loads(arguments_str)
                    result = await mcp_manager.call_tool(name, args)
                    if hasattr(result, "content") and isinstance(result.content, list):
                        texts = [c.text for c in result.content if hasattr(c, "text")]
                        result_str = "\n".join(texts)
                    else:
                        result_str = str(result)
                except Exception as e:
                    logger.error(f"[Session: {req.session_id}] Tool {name} failed: {e}")
                    result_str = f"Error: {e}"
                    
                logger.info(f"[Session: {req.session_id}] MCP tool {name} returned {len(result_str)} chars.")
                    
                messages.append({
                    "role": "tool",
                    "tool_call_id": tc["id"],
                    "name": name,
                    "content": result_str
                })
                
            # The loop continues with tools output added to messages
        except Exception as e:
            logger.exception("OpenAI stream error")
            yield ChatStreamEventAborted(code="PROVIDER_ERROR", message=str(e), retryable=True)
            break

async def _anthropic_stream(req: ChatRequestInput, profile: ApiProfile, model: str) -> AsyncGenerator[ChatStreamEvent, None]:
    client = AsyncAnthropic(api_key=profile.api_key, base_url=profile.base_url)
    
    system_content = _build_chat_system_content(req)
    
    raw_messages = []
    for msg in req.history:
        raw_messages.append((msg.role, msg.content))
        
    user_content = req.prompt.user.strip()
    raw_messages.append(("user", user_content if user_content else " "))
    
    # Merge consecutive messages
    messages = []
    for role, content in raw_messages:
        if messages and messages[-1]["role"] == role:
            messages[-1]["content"] += f"\n\n{content}"
        else:
            messages.append({"role": role, "content": content})
            
    if messages and messages[0]["role"] == "assistant":
        messages.insert(0, {"role": "user", "content": " "})

    while True:
        try:
            kwargs = {
                "model": model,
                "max_tokens": 1024,
                "system": system_content,
                "messages": messages,
                "stream": True,
            }
            
            # Anthropic tool format is slightly different
            tools = mcp_manager.get_all_tools()
            if tools:
                anthropic_tools = []
                for t in tools:
                    anthropic_tools.append({
                        "name": t["function"]["name"],
                        "description": t["function"]["description"],
                        "input_schema": t["function"]["parameters"]
                    })
                kwargs["tools"] = anthropic_tools

            response = await client.messages.create(**kwargs)
            
            accumulated_content = ""
            tool_calls = []
            current_tool_call = None
            
            async for event in response:
                if event.type == "message_start":
                    yield ChatStreamEventUsage(
                        tokens_in=event.message.usage.input_tokens,
                        tokens_out=0,
                    )
                elif event.type == "content_block_start":
                    if event.content_block.type == "tool_use":
                        current_tool_call = {
                            "id": event.content_block.id,
                            "name": event.content_block.name,
                            "input": ""
                        }
                elif event.type == "content_block_delta":
                    if event.delta.type == "text_delta":
                        accumulated_content += event.delta.text
                        yield ChatStreamEventDelta(text=event.delta.text)
                    elif event.delta.type == "thinking_delta":
                        yield ChatStreamEventReasoning(text=event.delta.thinking)
                    elif event.delta.type == "input_json_delta":
                        if current_tool_call:
                            current_tool_call["input"] += event.delta.partial_json
                elif event.type == "content_block_stop":
                    if current_tool_call:
                        tool_calls.append(current_tool_call)
                        current_tool_call = None
                elif event.type == "message_delta":
                    yield ChatStreamEventUsage(
                        tokens_in=0,
                        tokens_out=event.usage.output_tokens,
                    )
                    
            if not tool_calls:
                yield ChatStreamEventDone()
                break
                
            # Prepare the assistant message
            assistant_content = []
            if accumulated_content:
                assistant_content.append({"type": "text", "text": accumulated_content})
            for tc in tool_calls:
                try:
                    parsed_input = json.loads(tc["input"])
                except Exception:
                    parsed_input = {}
                assistant_content.append({
                    "type": "tool_use",
                    "id": tc["id"],
                    "name": tc["name"],
                    "input": parsed_input
                })
                
            messages.append({"role": "assistant", "content": assistant_content})
            
            # Execute tool calls
            tool_results = []
            for tc in tool_calls:
                name = tc["name"]
                arguments_str = tc["input"]
                logger.info(f"[Session: {req.session_id}] Executing MCP tool: {name} | Args: {arguments_str}")
                
                try:
                    args = json.loads(arguments_str)
                    result = await mcp_manager.call_tool(name, args)
                    if hasattr(result, "content") and isinstance(result.content, list):
                        texts = [c.text for c in result.content if hasattr(c, "text")]
                        result_str = "\n".join(texts)
                    else:
                        result_str = str(result)
                except Exception as e:
                    logger.error(f"[Session: {req.session_id}] Tool {name} failed: {e}")
                    result_str = f"Error: {e}"
                    
                logger.info(f"[Session: {req.session_id}] MCP tool {name} returned {len(result_str)} chars.")
                    
                tool_results.append({
                    "type": "tool_result",
                    "tool_use_id": tc["id"],
                    "content": result_str
                })
                
            messages.append({"role": "user", "content": tool_results})
            
        except Exception as e:
            logger.exception("Anthropic stream error")
            yield ChatStreamEventAborted(code="PROVIDER_ERROR", message=str(e), retryable=True)
            break

async def generate_title(req: TitleGenerateRequest) -> str:
    profile = next((p for p in req.cfg.profiles if p.id == req.profile_id), None)
    if not profile or not profile.enabled:
        raise ValueError("Profile not found or disabled")
    
    model = req.model or profile.default_model
    user_content = f"User text:\n<text>\n{req.user_text}\n</text>\n\nGenerate title in the EXACT SAME LANGUAGE as the text above:"
    
    if profile.provider == "openai":
        client = AsyncOpenAI(api_key=profile.api_key, base_url=profile.base_url)
        res = await client.chat.completions.create(
            model=model,
            messages=[
                {"role": "system", "content": TITLE_GENERATION_PROMPT},
                {"role": "user", "content": user_content}
            ]
        )
        return res.choices[0].message.content or ""
    elif profile.provider == "anthropic":
        client = AsyncAnthropic(api_key=profile.api_key, base_url=profile.base_url)
        res = await client.messages.create(
            model=model,
            max_tokens=1024,
            system=TITLE_GENERATION_PROMPT,
            messages=[{"role": "user", "content": user_content}]
        )
        return res.content[0].text
    raise ValueError(f"Unknown provider: {profile.provider}")

async def extract_memories(req: MemoryExtractRequest) -> dict:
    profile = next((p for p in req.cfg.profiles if p.id == req.profile_id), None)
    if not profile or not profile.enabled:
        raise ValueError("Profile not found or disabled")
        
    model = req.model or profile.default_model
    
    user_content = f"User said: {req.user_text}\n\nAssistant replied: {req.assistant_text}\n\nExtract new memories:"
    
    if profile.provider == "openai":
        client = AsyncOpenAI(api_key=profile.api_key, base_url=profile.base_url)
        messages = [{"role": "system", "content": MEMORY_EXTRACTION_PROMPT}]
        for msg in req.history:
            messages.append({"role": msg.role, "content": msg.content})
        messages.append({"role": "user", "content": user_content})
        
        res = await client.chat.completions.create(
            model=model,
            response_format={"type": "json_object"},
            messages=messages
        )
        content = res.choices[0].message.content or "{}"
        return _parse_json_markdown(content)
        
    elif profile.provider == "anthropic":
        client = AsyncAnthropic(api_key=profile.api_key, base_url=profile.base_url)
        messages = []
        raw_messages = [(msg.role, msg.content) for msg in req.history]
        raw_messages.append(("user", user_content))
        
        for role, content in raw_messages:
            if messages and messages[-1]["role"] == role:
                messages[-1]["content"] += f"\n\n{content}"
            else:
                messages.append({"role": role, "content": content})
                
        if messages and messages[0]["role"] == "assistant":
            messages.insert(0, {"role": "user", "content": " "})
            
        messages.append({"role": "assistant", "content": "{"})
        
        res = await client.messages.create(
            model=model,
            max_tokens=1024,
            system=MEMORY_EXTRACTION_PROMPT,
            messages=messages
        )
        content = "{" + res.content[0].text
        return _parse_json_markdown(content)
        
    raise ValueError(f"Unknown provider: {profile.provider}")

def _parse_json_markdown(text: str) -> dict:
    t = text.strip()
    if t.startswith("```json"):
        t = t[7:].strip()
    elif t.startswith("```"):
        t = t[3:].strip()
    if t.endswith("```"):
        t = t[:-3].strip()
    try:
        return json.loads(t)
    except Exception:
        return {"memories": []}
