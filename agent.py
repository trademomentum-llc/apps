"""OpenClaw deep agent — Opus 4.6 with openclaw subagent."""

from deepagents import SubAgent, create_deep_agent

openclaw = SubAgent(
    name="openclaw",
    description=(
        "OpenClaw: autonomous web crawler and code analysis agent. "
        "Crawls URLs, extracts structured data, analyzes codebases, "
        "and returns findings as structured reports."
    ),
    system_prompt=(
        "You are OpenClaw, a focused research and crawling subagent.\n\n"
        "Your capabilities:\n"
        "- Crawl and scrape web pages for structured data\n"
        "- Analyze source code files and repositories\n"
        "- Extract, summarize, and report findings\n"
        "- Execute shell commands for data retrieval\n\n"
        "Guidelines:\n"
        "- Be thorough but concise in your reports\n"
        "- Return structured output (JSON or markdown tables) when possible\n"
        "- Always cite sources and file paths\n"
        "- If a crawl fails, report the error and suggest alternatives"
    ),
)

agent = create_deep_agent(
    model="claude-opus-4-6",
    subagents=[openclaw],
    system_prompt=(
        "You are the master agent coordinating the OpenClaw system. "
        "Delegate crawling, scraping, and research tasks to the 'openclaw' subagent. "
        "Use your built-in tools for file operations, planning, and orchestration."
    ),
)

if __name__ == "__main__":
    print("OpenClaw deep agent created successfully.")
    print(f"Model: claude-opus-4-6")
    print(f"Subagents: [openclaw]")
    print(f"Agent type: {type(agent).__name__}")
