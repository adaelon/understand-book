@echo off
setlocal
set "VIDEO_BUILD_BIN=%UNDERSTAND_BOOK_BUILD_EXE%"
if not defined VIDEO_BUILD_BIN goto unavailable
if not exist "%VIDEO_BUILD_BIN%" goto unavailable
"%VIDEO_BUILD_BIN%" %*
exit /b %ERRORLEVEL%
:unavailable
1>&2 echo Operator video processing needs UNDERSTAND_BOOK_BUILD_EXE. Cloud Windows Setup does not include processing tools.
exit /b 2
