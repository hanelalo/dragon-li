import asyncio
from mcp_client import mcp_manager
from mcp.client.stdio import StdioServerParameters

async def main():
    try:
        await mcp_manager.connect(
            "sqlite",
            StdioServerParameters(
                command="uvx",
                args=["mcp-server-sqlite", "--db-path", "test.db"]
            )
        )
        print("Tools:", mcp_manager.get_all_tools())
    except Exception as e:
        print("Error:", e)
    finally:
        await mcp_manager.close()

asyncio.run(main())
