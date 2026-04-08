import json
from typing import List, Optional, Literal, Union, Dict, Any
from pydantic import BaseModel, Field

class ChatPromptLayer(BaseModel):
    system: str
    runtime: str
    memory: str
    user: str

class ChatMessageContext(BaseModel):
    role: str
    content: str

class ApiProfile(BaseModel):
    id: str
    name: str
    provider: str
    base_url: str
    api_key: str
    default_model: str
    enabled: bool
    created_at: str
    updated_at: str

class ToolsConfig(BaseModel):
    brave_search_api_key: Optional[str] = None

class ApiProfilesConfig(BaseModel):
    profiles: List[ApiProfile]
    tools: ToolsConfig = Field(default_factory=ToolsConfig)

class ChatRequestInput(BaseModel):
    profile_id: str
    request_id: str
    session_id: Optional[str] = None
    model: Optional[str] = None
    prompt: ChatPromptLayer
    enable_web_search: bool = False
    history: List[ChatMessageContext] = Field(default_factory=list)
    # The config is passed from Rust since Python doesn't load it directly here
    cfg: Optional[ApiProfilesConfig] = None

class ChatStreamEventDelta(BaseModel):
    type: Literal["delta"] = "delta"
    text: str

class ChatStreamEventReasoning(BaseModel):
    type: Literal["reasoning"] = "reasoning"
    text: str

class ChatStreamEventUsage(BaseModel):
    type: Literal["usage"] = "usage"
    tokens_in: int
    tokens_out: int

class ChatStreamEventDone(BaseModel):
    type: Literal["done"] = "done"

class ChatStreamEventAborted(BaseModel):
    type: Literal["aborted"] = "aborted"
    code: str
    message: str
    retryable: bool

ChatStreamEvent = Union[
    ChatStreamEventDelta,
    ChatStreamEventReasoning,
    ChatStreamEventUsage,
    ChatStreamEventDone,
    ChatStreamEventAborted,
]

def event_to_json(event: ChatStreamEvent) -> str:
    # Match Rust snake_case tag for enum
    # We output exactly like { "type": "delta", "text": "..." }
    return json.dumps(event.model_dump(exclude_none=True), separators=(',', ':'))

class TitleGenerateRequest(BaseModel):
    profile_id: str
    model: Optional[str] = None
    user_text: str
    cfg: Optional[ApiProfilesConfig] = None

class TitleGenerateResponse(BaseModel):
    title: str

class MemoryExtractRequest(BaseModel):
    profile_id: str
    model: Optional[str] = None
    session_id: str
    user_text: str
    assistant_text: str
    history: List[ChatMessageContext] = Field(default_factory=list)
    cfg: Optional[ApiProfilesConfig] = None

class MemoryCandidate(BaseModel):
    type_: str
    summary: str
    evidence: str
    tags: List[str]
    confidence: float

class MemoryExtractResponse(BaseModel):
    memories: List[MemoryCandidate]
