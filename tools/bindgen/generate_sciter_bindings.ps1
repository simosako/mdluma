param(
    [string]$SdkInclude = "$PSScriptRoot\..\..\vendor\sciter-js-sdk-main\include",
    [string]$Output = "$PSScriptRoot\..\..\src\sciter\generated_sciter_bindings.rs"
)

$ErrorActionPreference = "Stop"

$bindgen = Get-Command bindgen -ErrorAction SilentlyContinue
if (-not $bindgen) {
    $bindgen = "$env:USERPROFILE\.cargo\bin\bindgen.exe"
    if (-not (Test-Path $bindgen)) {
        throw "bindgen not found in PATH or at $bindgen. Install it with: rtk cargo install bindgen-cli"
    }
} else {
    $bindgen = $bindgen.Source
}

$wrapper = "$PSScriptRoot\sciter_wrapper.h"
if (-not (Test-Path $wrapper)) {
    throw "wrapper header not found: $wrapper"
}

if (-not (Test-Path $SdkInclude)) {
    throw "Sciter SDK include path not found: $SdkInclude"
}

& $bindgen $wrapper `
  --allowlist-type "_ISciterAPI|ISciterAPI|VALUE|SCITER_VALUE" `
  --allowlist-var "SCITER_SET_INIT_SCRIPT|SCITER_APP_INIT|SCITER_APP_LOOP|SCITER_API_VERSION|SCITER_VERSION_0|SCITER_VERSION_1|SCITER_VERSION_2|SCITER_VERSION_3" `
  --allowlist-function "SciterAPI" `
  --no-layout-tests `
  --use-core `
  -- "-I$SdkInclude" -DWINDOWS | Out-File -Encoding utf8 $Output

Write-Host "Generated: $Output"
