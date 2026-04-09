import logging
import json
from datetime import datetime
from typing import AsyncGenerator

from openai import AsyncOpenAI
from anthropic import AsyncAnthropic

from core.models import (
    ChatRequestInput,
    ChatStreamEvent,
    ChatStreamEventAborted,
    event_to_json,
    ApiProfile,
    TitleGenerateRequest,
    MemoryExtractRequest
)
from core.prompts import TITLE_GENERATION_PROMPT, MEMORY_EXTRACTION_PROMPT, CHAT_SYSTEM_PROMPT_TEMPLATE

from llm.openai_provider import _openai_stream
from llm.anthropic_provider import _anthropic_stream

logger = logging.getLogger("uvicorn.error")

def _build_chat_system_content(req: ChatRequestInput) -> str:
    memory_part = req.prompt.memory.strip()
    memory_section = f"# Context (Injected Memories)\n{memory_part}\n\n" if memory_part else ""
    current_time = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    base_system = CHAT_SYSTEM_PROMPT_TEMPLATE.format(memory_section=memory_section, current_time=current_time)
    system_parts = [base_system, req.prompt.system, req.prompt.runtime]
    
    return "\n\n".join(p for p in system_parts if p.strip())

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
        system_content = _build_chat_system_content(req)

        if profile.provider == "openai":
            async for event in _openai_stream(req, profile, model, system_content):
                yield f"data: {event_to_json(event)}\n\n"
        elif profile.provider == "anthropic":
            async for event in _anthropic_stream(req, profile, model, system_content):
                yield f"data: {event_to_json(event)}\n\n"
        else:
            yield f"data: {event_to_json(ChatStreamEventAborted(code='INVALID_REQUEST', message=f'Unknown provider: {profile.provider}', retryable=False))}\n\n"
            return

    except Exception as e:
        logger.exception("LLM Stream failed")
        yield f"data: {event_to_json(ChatStreamEventAborted(code='PROVIDER_SERVER_ERROR', message=str(e), retryable=True))}\n\n"


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
