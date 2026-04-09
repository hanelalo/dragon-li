import logging
import json
from typing import AsyncGenerator

from anthropic import AsyncAnthropic
from core.models import (
    ChatRequestInput,
    ChatStreamEvent,
    ChatStreamEventDelta,
    ChatStreamEventReasoning,
    ChatStreamEventUsage,
    ChatStreamEventDone,
    ChatStreamEventAborted,
    ApiProfile,
)
from tools.registry import get_tools_for_anthropic, execute_tool

logger = logging.getLogger("uvicorn.error")

async def _anthropic_stream(req: ChatRequestInput, profile: ApiProfile, model: str, system_content: str) -> AsyncGenerator[ChatStreamEvent, None]:
    client = AsyncAnthropic(api_key=profile.api_key, base_url=profile.base_url)
    
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
            
            anthropic_tools = get_tools_for_anthropic(req.enable_web_search, req.cfg)
            if anthropic_tools:
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
                
                result_str = await execute_tool(name, arguments_str, req.cfg, str(req.session_id))
                    
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