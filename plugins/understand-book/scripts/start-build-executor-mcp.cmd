@echo off
setlocal
set "BUILD_EXECUTOR_BIN=%UNDERSTAND_BOOK_BUILD_EXE%"
if not defined BUILD_EXECUTOR_BIN goto unavailable
if not exist "%BUILD_EXECUTOR_BIN%" goto unavailable
"%BUILD_EXECUTOR_BIN%" executor.mcp --bootstrap-version automatic_build_executor_bootstrap.v3 --protocol-generation automatic_build_executor_session.v3
exit /b %ERRORLEVEL%
:unavailable
1>&2 echo This is an operator-only build plugin. Set UNDERSTAND_BOOK_BUILD_EXE to the maintained build tool. Cloud Windows Setup does not include it.
exit /b 2
