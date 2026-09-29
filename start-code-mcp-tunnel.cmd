@echo off
title Understand Book Code MCP Tunnel
powershell.exe -NoLogo -NoProfile -File "%~dp0scripts\code-mcp-tunnel.ps1"
if errorlevel 1 (
  echo.
  echo Startup failed. See the message above.
  pause
)
