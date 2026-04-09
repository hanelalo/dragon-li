import httpx
import json
import logging
import trafilatura
from datetime import datetime

logger = logging.getLogger("uvicorn.error")

async def execute_web_search(query: str, api_key: str) -> str:
    logger.info(f"Executing web search for query: {query}")
    url = "https://api.search.brave.com/res/v1/web/search"
    headers = {"X-Subscription-Token": api_key, "Accept": "application/json"}
    try:
        async with httpx.AsyncClient() as client:
            res = await client.get(url, headers=headers, params={"q": query, "count": 5})
            res.raise_for_status()
            data = res.json()
            
        results = data.get("web", {}).get("results", [])[:5]
        cleaned = [{"t": r.get("title"), "u": r.get("url"), "d": r.get("description")} for r in results]
        logger.info(f"Web search completed, found {len(cleaned)} results.")
        return json.dumps(cleaned, ensure_ascii=False)
    except Exception as e:
        logger.error(f"Web search failed: {e}")
        raise

async def fetch_webpage(url: str) -> str:
    """
    Fetch the specified URL and extract the core text content.
    """
    logger.info(f"Fetching webpage: {url}")
    headers = {
        "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    }
    try:
        async with httpx.AsyncClient(timeout=10.0, follow_redirects=True) as client:
            res = await client.get(url, headers=headers)
            res.raise_for_status()
            logger.info(f"Successfully downloaded webpage: {url}, HTML length: {len(res.text)}")
            
            extracted = trafilatura.extract(
                res.text,
                include_links=True,
                include_formatting=True
            )
            
            if not extracted:
                logger.warning(f"No core text found for {url}. Raw HTML length: {len(res.text)}")
                return f"Successfully fetched but no core text found. Might be a SPA or dynamic page requiring JS.\n\nRaw HTML length: {len(res.text)}"
                
            max_chars = 15000
            if len(extracted) > max_chars:
                logger.info(f"Extracted text too long ({len(extracted)} chars), truncating to {max_chars}.")
                return extracted[:max_chars] + "\n\n...[Content too long, truncated]..."
            
            logger.info(f"Successfully extracted {len(extracted)} chars of core text from {url}.")
            return extracted
    except httpx.HTTPStatusError as e:
        logger.error(f"HTTP Error fetching {url}: {e.response.status_code}")
        return f"Request failed: HTTP {e.response.status_code}"
    except Exception as e:
        logger.error(f"Error fetching webpage {url}: {str(e)}")
        return f"Error fetching webpage: {str(e)}"

def get_current_time() -> str:
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")
