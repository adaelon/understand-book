@echo off
setlocal
set "BOOK_MCP_BIN=%UNDERSTAND_BOOK_MCP_BIN%"
if not defined BOOK_MCP_BIN goto unavailable
if not exist "%BOOK_MCP_BIN%" goto unavailable
"%BOOK_MCP_BIN%" %*
exit /b %ERRORLEVEL%
:unavailable
1>&2 echo This local maintenance MCP needs UNDERSTAND_BOOK_MCP_BIN. For cloud reading MCP, use the account's remote MCP connection.
exit /b 2
