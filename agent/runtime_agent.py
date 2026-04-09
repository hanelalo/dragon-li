#!/usr/bin/env python3
"""Minimal runtime agent for desktop-runtime-core bootstrap."""

import time
_START_TIME = time.time()

import argparse
import json
import sys
import os
import asyncio
import logging

from fastapi import FastAPI
import uvicorn
from contextlib import asynccontextmanager

from agent_mcp.client import mcp_manager
from skills.manager import skill_manager
from api.mcp import router as mcp_router, load_mcp_from_db
from api.chat import router as chat_router

logger = logging.getLogger("uvicorn.error")

@asynccontextmanager
async def lifespan(app: FastAPI):
    # 将 MCP 和 Skill 加载放到后台任务中，避免阻塞服务启动
    asyncio.create_task(asyncio.to_thread(skill_manager.scan_skills_directory))
    asyncio.create_task(load_mcp_from_db())
    logger.info(f"====== Python Agent Started in {time.time() - _START_TIME:.3f} seconds ======")
    yield
    await mcp_manager.close()

app = FastAPI(title="Dragon-Li Runtime Agent", lifespan=lifespan)

app.include_router(mcp_router, prefix="/v1/mcp", tags=["mcp"])
app.include_router(chat_router, prefix="/v1/chat", tags=["chat"])

@app.get("/health")
def health_check():
    return {"status": "ok"}

def run_server(uds_path: str) -> int:
    if not uds_path:
        print(json.dumps({"ok": False, "error": "--uds is required for --serve"}))
        return 1
    
    # Run uvicorn with the unix domain socket
    uvicorn.run(app, uds=uds_path)
    return 0

def main() -> int:
    parser = argparse.ArgumentParser(description="Dragon-Li runtime agent")
    parser.add_argument("--serve", action="store_true")
    parser.add_argument("--uds", type=str, help="Path to the Unix Domain Socket")
    parser.add_argument("--db-path", type=str, help="Path to the SQLite database")
    args = parser.parse_args()
    
    if args.db_path:
        os.environ["DRAGON_LI_DB_PATH"] = args.db_path

    if args.serve:
        return run_server(args.uds)

    print(json.dumps({"ok": False, "error": "No mode selected"}))
    return 1

if __name__ == "__main__":
    raise SystemExit(main())