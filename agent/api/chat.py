from fastapi import APIRouter, HTTPException
from fastapi.responses import StreamingResponse
import logging

from core.models import ChatRequestInput, TitleGenerateRequest, TitleGenerateResponse, MemoryExtractRequest, MemoryExtractResponse
from llm.provider import chat_stream_generator, generate_title, extract_memories

logger = logging.getLogger("uvicorn.error")

router = APIRouter()

@router.post("/stream")
async def chat_stream(req: ChatRequestInput):
    logger.info(f"Received chat stream request. enable_web_search: {req.enable_web_search}")
    return StreamingResponse(
        chat_stream_generator(req),
        media_type="text/event-stream",
    )

@router.post("/summarize_title")
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

@router.post("/memory/extract")
async def memory_extract(req: MemoryExtractRequest) -> MemoryExtractResponse:
    try:
        data = await extract_memories(req)
        memories = data.get("memories", [])
        return MemoryExtractResponse(memories=memories)
    except Exception as e:
        logger.exception("Failed to extract memory")
        raise HTTPException(status_code=500, detail=str(e))
