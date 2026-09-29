[CmdletBinding(DefaultParameterSetName = 'Start')]
param(
    [Parameter(ParameterSetName = 'Install', Mandatory = $true)]
    [switch]$InstallCurrentCredential,

    [Parameter(ParameterSetName = 'Verify', Mandatory = $true)]
    [switch]$VerifyStoredCredential,

    [string]$CredentialPath = (Join-Path $env:APPDATA 'OpenAI/understand-book-code/control-plane-key.dpapi')
)

$ErrorActionPreference = 'Stop'
$tunnelClient = Join-Path $env:LOCALAPPDATA 'OpenAI/tunnel-client/v0.0.14/tunnel-client.exe'
$profilePath = Join-Path $env:APPDATA 'tunnel-client/understand-book-code.yaml'
$healthUrlFile = Join-Path $env:USERPROFILE '.local/state/tunnel-client/health/understand-book-code.url'

function Protect-CredentialFile([string]$Path) {
    $currentSid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
    $systemSid = [System.Security.Principal.SecurityIdentifier]::new(
        [System.Security.Principal.WellKnownSidType]::LocalSystemSid,
        $null
    )
    $acl = [System.Security.AccessControl.FileSecurity]::new()
    $acl.SetOwner($currentSid)
    $acl.SetAccessRuleProtection($true, $false)
    $acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new(
        $currentSid,
        [System.Security.AccessControl.FileSystemRights]::FullControl,
        [System.Security.AccessControl.AccessControlType]::Allow
    ))
    $acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new(
        $systemSid,
        [System.Security.AccessControl.FileSystemRights]::FullControl,
        [System.Security.AccessControl.AccessControlType]::Allow
    ))
    Set-Acl -LiteralPath $Path -AclObject $acl
}

function Read-StoredCredential([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "No stored tunnel credential. Run this in the PowerShell session that has CONTROL_PLANE_API_KEY: & '$PSCommandPath' -InstallCurrentCredential"
    }
    $encrypted = (Get-Content -Raw -LiteralPath $Path).Trim()
    if ([string]::IsNullOrWhiteSpace($encrypted)) {
        throw "The encrypted credential file is empty: $Path"
    }
    $secure = ConvertTo-SecureString $encrypted
    $pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
    try {
        return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    }
    finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
    }
}

if ($InstallCurrentCredential) {
    if ([string]::IsNullOrWhiteSpace($env:CONTROL_PLANE_API_KEY)) {
        throw 'CONTROL_PLANE_API_KEY is not available in this PowerShell session. Run setup from the session that currently holds the key.'
    }
    $parent = Split-Path -Parent $CredentialPath
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    $secure = ConvertTo-SecureString $env:CONTROL_PLANE_API_KEY -AsPlainText -Force
    ConvertFrom-SecureString $secure | Set-Content -LiteralPath $CredentialPath -Encoding ASCII -NoNewline
    Protect-CredentialFile $CredentialPath
    Write-Host "Saved the encrypted tunnel credential for the current Windows user: $CredentialPath"
    exit 0
}

if ($VerifyStoredCredential) {
    $credential = Read-StoredCredential $CredentialPath
    try {
        if ([string]::IsNullOrWhiteSpace($credential)) {
            throw 'The decrypted credential is empty.'
        }
        Write-Host 'The encrypted tunnel credential can be read by the current Windows user.'
    }
    finally {
        $credential = $null
    }
    exit 0
}

if (-not (Test-Path -LiteralPath $tunnelClient -PathType Leaf)) {
    throw "tunnel-client was not found: $tunnelClient"
}
if (-not (Test-Path -LiteralPath $profilePath -PathType Leaf)) {
    throw "The tunnel profile was not found: $profilePath"
}

# A live ready endpoint means the same profile is already running; do not start a duplicate poller.
if (Test-Path -LiteralPath $healthUrlFile -PathType Leaf) {
    try {
        $healthBase = (Get-Content -Raw -LiteralPath $healthUrlFile).TrimEnd('/')
        $ready = Invoke-WebRequest -UseBasicParsing -Uri "$healthBase/readyz" -TimeoutSec 2
        if ($ready.StatusCode -eq 200 -and $ready.Content.Trim() -eq 'ready') {
            Write-Host "The Understand Book read-only code tunnel is already running: $healthBase/ui"
            exit 0
        }
    }
    catch {
        # The URL file may be left from an earlier boot. Starting below rewrites it.
    }
}

$credential = Read-StoredCredential $CredentialPath
try {
    $env:CONTROL_PLANE_API_KEY = $credential
    Write-Host 'Starting the Understand Book read-only code tunnel. Keep this window open.'
    & $tunnelClient run --profile understand-book-code
    exit $LASTEXITCODE
}
finally {
    Remove-Item Env:CONTROL_PLANE_API_KEY -ErrorAction SilentlyContinue
    $credential = $null
}
