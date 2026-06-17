# AGENTS.md

Agent collaboration rules for Darkbluff. `CLAUDE.md` is a symlink to this file.

## Basic Rules

- Prefer Chinese when communicating with the maintainer.
- Use English snake_case for code identifiers, filenames, command names, and data IDs.
- Use Chinese by default for player-facing text and design notes.
- Read the relevant design docs before making changes to avoid conflicts with existing rules.
- Keep changes small and focused; do not delete user-owned content unless explicitly asked.
- When old mechanics remain in the docs, search globally and update related documents consistently.

## Working Style

Write the least necessary content. Do not add complexity that was not requested.

Priority:

1. If it does not need to exist, do not write it.
2. If the standard library or platform can solve it, use that.
3. If existing dependencies or designs can be reused, reuse them first.
4. If one line is clear enough, do not write ten.
5. When something must be added, implement the smallest correct version for the current need.

Do not sacrifice input validation, data safety, error handling, security, or accessibility just to write less.

When working:

- If uncertain, state the doubt and ask; do not silently assume.
- If there are multiple interpretations or tradeoffs, list them and give a recommendation.
- For multi-step tasks, give a short plan and explain how each step will be verified.
- Only change lines directly related to the current goal; do not casually refactor or clean unrelated code.
- Match the existing style. If you notice an unrelated issue, mention it instead of fixing it unasked.
- Verify changes when possible. If verification is not possible, explain why and note the risk.

## Commit Message

Format: `<type>(<scope>): <short Chinese description>`

- Common type: `feat` / `fix` / `docs` / `refactor` / `build` / `test` / `chore`
- Common scope: `project` / `doc` / `gameplay` / `command` / `narrative` / `data` / `save` / `engine` / `ui` / `build`
- Description rules: Chinese, starts with a verb, concise and clear, no period, preferably within 50 Chinese characters.

Examples:

```text
feat(project): 初始化设计文档
feat(doc): 初始化 README
fix(doc): 修正 map 回滚说明
feat(command): 新增 map 指令设计
build(project): 初始化 Rust 工程
```
