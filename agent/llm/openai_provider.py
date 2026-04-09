import logging
import json
from typing import AsyncGenerator

from openai import AsyncOpenAI
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
from tools.registry import get_tools_for_openai, execute_tool

logger = logging.getLogger("uvicorn.error")

async def _openai_stream(req: ChatRequestInput, profile: ApiProfile, model: str, system_content: str) -> AsyncGenerator[ChatStreamEvent, None]:
    client = AsyncOpenAI(api_key=profile.api_key, base_url=profile.base_url)
    
    messages = []
    
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
            tools = get_tools_for_openai(req.enable_web_search, req.cfg)

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
                
                result_str = await execute_tool(name, arguments_str, req.cfg, str(req.session_id))
                    
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