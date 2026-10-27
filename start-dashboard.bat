@echo off
setlocal EnableExtensions EnableDelayedExpansion
cd /d "%~dp0"

set "PROJECT_DIR=%CD%"
if exist "%PROJECT_DIR%\.understand-anything\knowledge-graph.json" (
  set "UA_DIR=%PROJECT_DIR%\.understand-anything"
) else (
  set "UA_DIR=%PROJECT_DIR%\.ua"
)
set "GRAPH_FILE=%UA_DIR%\knowledge-graph.json"
set "URL_FILE=%UA_DIR%\dashboard.url"
set "OUT_LOG=%UA_DIR%\dashboard.out.log"
set "NO_OPEN=0"
set "FORCE_NEW=0"
set "CHECK_ONLY=0"

:parse_args
if "%~1"=="" goto args_done
if /i "%~1"=="--no-open" (
  set "NO_OPEN=1"
) else if /i "%~1"=="--force-new" (
  set "FORCE_NEW=1"
) else if /i "%~1"=="--check" (
  set "CHECK_ONLY=1"
) else (
  echo Unknown option: %~1
  echo Supported options: --no-open --force-new --check
  exit /b 2
)
shift
goto parse_args

:args_done
if not exist "%GRAPH_FILE%" (
  echo [ERROR] Knowledge graph not found:
  echo         %GRAPH_FILE%
  echo Run /understand before starting the dashboard.
  call :pause_if_interactive
  exit /b 1
)

if "%FORCE_NEW%"=="0" call :find_running_dashboard
if defined DASHBOARD_URL (
  >"%URL_FILE%" echo !DASHBOARD_URL!
  echo Dashboard is already running:
  echo !DASHBOARD_URL!
  if "%CHECK_ONLY%"=="0" if "%NO_OPEN%"=="0" start "" "!DASHBOARD_URL!"
  exit /b 0
)

call :find_dashboard_dir
if not defined DASHBOARD_DIR (
  echo [ERROR] Understand Anything dashboard package was not found.
  echo Reinstall or rebuild the Understand Anything plugin, then try again.
  call :pause_if_interactive
  exit /b 1
)

set "VITE_CMD=%DASHBOARD_DIR%\node_modules\.bin\vite.cmd"
if not exist "%VITE_CMD%" (
  echo [ERROR] Dashboard dependencies are not installed:
  echo         %VITE_CMD%
  echo Run pnpm install in the Understand Anything plugin directory.
  call :pause_if_interactive
  exit /b 1
)

for /f "delims=" %%P in ('powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$used=[Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners().Port; $port=5173; while($port -le 5199) { if($used -notcontains $port) { Write-Output $port; exit 0 }; $port++ }; exit 1"') do set "DASHBOARD_PORT=%%P"
if not defined DASHBOARD_PORT (
  echo [ERROR] No free dashboard port was found in the range 5173-5199.
  call :pause_if_interactive
  exit /b 1
)

for /f "delims=" %%T in ('powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "[guid]::NewGuid().ToString('N')"') do set "UNDERSTAND_ACCESS_TOKEN=%%T"
if not defined UNDERSTAND_ACCESS_TOKEN (
  echo [ERROR] Failed to generate the dashboard access token.
  call :pause_if_interactive
  exit /b 1
)

set "DASHBOARD_URL=http://127.0.0.1:!DASHBOARD_PORT!/?token=!UNDERSTAND_ACCESS_TOKEN!"

if "%CHECK_ONLY%"=="1" (
  echo Dashboard startup check passed.
  echo Project:   %PROJECT_DIR%
  echo Dashboard: %DASHBOARD_DIR%
  echo Port:      !DASHBOARD_PORT!
  exit /b 0
)

>"%URL_FILE%" echo !DASHBOARD_URL!
set "GRAPH_DIR=%PROJECT_DIR%"
if "%NO_OPEN%"=="1" set "BROWSER=none"

echo Starting Understand Anything Dashboard...
echo Project: %PROJECT_DIR%
echo URL:     !DASHBOARD_URL!
echo.
echo Keep this window open while using the dashboard.
echo Press Ctrl+C or close this window to stop it.
echo.

pushd "%DASHBOARD_DIR%"
if errorlevel 1 (
  echo [ERROR] Cannot enter the dashboard directory:
  echo         %DASHBOARD_DIR%
  call :pause_if_interactive
  exit /b 1
)

call "node_modules\.bin\vite.cmd" --host 127.0.0.1 --port !DASHBOARD_PORT! --strictPort
set "DASHBOARD_EXIT=%ERRORLEVEL%"
popd
echo.
if not "%DASHBOARD_EXIT%"=="0" (
  echo [ERROR] Dashboard exited with code %DASHBOARD_EXIT%.
  call :pause_if_interactive
)
exit /b %DASHBOARD_EXIT%

:find_running_dashboard
set "DASHBOARD_URL="
set "UA_DASHBOARD_URL_FILE=%URL_FILE%"
set "UA_DASHBOARD_OUT_LOG=%OUT_LOG%"
for /f "delims=" %%U in ('powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$files=@($env:UA_DASHBOARD_URL_FILE,$env:UA_DASHBOARD_OUT_LOG); foreach($file in $files) { if([string]::IsNullOrWhiteSpace($file) -or -not (Test-Path -LiteralPath $file)) { continue }; try { $stream=[IO.File]::Open($file,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite); $reader=[IO.StreamReader]::new($stream); $text=$reader.ReadToEnd(); $reader.Dispose(); $stream.Dispose() } catch { continue }; $matches=[regex]::Matches($text,'http://127\.0\.0\.1:\d+/\?token=[0-9a-f]+'); for($i=$matches.Count-1; $i -ge 0; $i--) { $url=$matches[$i].Value; $graph=$url -replace '/\?token=','/knowledge-graph.json?token='; try { $response=Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 -Uri $graph; if($response.StatusCode -eq 200) { Write-Output $url; exit 0 } } catch {} } }"') do set "DASHBOARD_URL=%%U"
exit /b 0

:find_dashboard_dir
set "DASHBOARD_DIR="
if defined CLAUDE_PLUGIN_ROOT if exist "%CLAUDE_PLUGIN_ROOT%\packages\dashboard\package.json" set "DASHBOARD_DIR=%CLAUDE_PLUGIN_ROOT%\packages\dashboard"
if defined DASHBOARD_DIR exit /b 0

if exist "%USERPROFILE%\.understand-anything\repo\understand-anything-plugin\packages\dashboard\package.json" set "DASHBOARD_DIR=%USERPROFILE%\.understand-anything\repo\understand-anything-plugin\packages\dashboard"
if defined DASHBOARD_DIR exit /b 0

if exist "%USERPROFILE%\.understand-anything-plugin\packages\dashboard\package.json" set "DASHBOARD_DIR=%USERPROFILE%\.understand-anything-plugin\packages\dashboard"
if defined DASHBOARD_DIR exit /b 0

if exist "%USERPROFILE%\.codex\understand-anything\understand-anything-plugin\packages\dashboard\package.json" set "DASHBOARD_DIR=%USERPROFILE%\.codex\understand-anything\understand-anything-plugin\packages\dashboard"
if defined DASHBOARD_DIR exit /b 0

if exist "%USERPROFILE%\understand-anything\understand-anything-plugin\packages\dashboard\package.json" set "DASHBOARD_DIR=%USERPROFILE%\understand-anything\understand-anything-plugin\packages\dashboard"
exit /b 0

:pause_if_interactive
if "%NO_OPEN%"=="0" pause
exit /b 0
