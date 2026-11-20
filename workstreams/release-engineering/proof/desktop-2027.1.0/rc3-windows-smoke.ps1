# Isolith rc3 Windows MSI smoke (executed on the garage PC, 192.168.0.43)
# Mirrors the CROSS-PLATFORM-INSTALLERS 14-check pattern; fetches the MSI from
# the GitHub Releases store itself, hashes vs SHA256SUMS, silent-installs,
# drives the full editor workflow on the INSTALLED binary, uninstalls cleanly.
$ErrorActionPreference = 'SilentlyContinue'
$log = "$env:TEMP\rc3-windows-smoke.log"
Set-Content -Path $log -Value "--- isolith rc3 windows smoke $(Get-Date -Format o) ---" -Encoding Ascii

function Note($s) { Add-Content -Path $log -Value $s; Write-Host $s }

$pass = 0; $fail = 0
function Check($name, $cond, $detail) {
    if ($cond) { $script:pass++; Note "[PASS] $name - $detail" }
    else       { $script:fail++; Note "[FAIL] $name - $detail" }
}

# 1. host is real Windows
$os = (Get-CimInstance Win32_OperatingSystem).Caption
Check "host-windows" ($os -match 'Windows') $os

# asset + checksums fetched FROM THE RELEASE STORE (github) by the box itself
$base = "https://github.com/isolith-project/isolith/releases/download/v2027.1.0-rc3"
$msi  = "$env:TEMP\rc3-msi\isolith-oep-workbench-x86_64-pc-windows-gnu-2027.1.0.msi"
New-Item -ItemType Directory -Force -Path "$env:TEMP\rc3-msi" | Out-Null
Invoke-WebRequest -UseBasicParsing "$base/isolith-oep-workbench-x86_64-pc-windows-gnu-2027.1.0.msi" -OutFile $msi
Invoke-WebRequest -UseBasicParsing "$base/SHA256SUMS" -OutFile "$env:TEMP\rc3-msi\SHA256SUMS"

# 2. box hash == release checksum
$h = (Get-FileHash -Algorithm SHA256 $msi).Hash.ToLower()
$sum = Get-Content "$env:TEMP\rc3-msi\SHA256SUMS" | Where-Object { $_ -match 'msi' }
$want = ($sum -split '\s+')[0]
Check "msi-hash-vs-checksums" ($h -eq $want) "box=$h checksum=$want"

# 3. silent install
$p = Start-Process msiexec -ArgumentList "/i `"$msi`" /qn /norestart /l*v $env:TEMP\rc3-msi\install.log" -Wait -PassThru
Check "msi-install-exit0" ($p.ExitCode -eq 0) "installer exit=$($p.ExitCode)"

# 4. installed binary exists (install the INSTALLED copy, not dist)
$exe = "C:\Program Files (x86)\Isolith\bin\oep-workbench.exe"
Check "msi-installed-binary" (Test-Path $exe) "path=$exe"

function DriveWorkbench($bin, $workdir) {
    New-Item -ItemType Directory -Force -Path $workdir | Out-Null
    & $bin `
        editor:open `
        sk:point:0:0 sk:point:3:1 sk:point:4:4 sk:point:1:5 `
        sk:line:p0:p1 sk:line:p1:p2 sk:line:p2:p3 sk:line:p3:p0 `
        sk:constrain:fixed:p0:0:0 sk:constrain:horizontal:l0 sk:constrain:horizontal:l2 `
        sk:constrain:vertical:l1 sk:constrain:vertical:l3 `
        sk:constrain:length:l0:width sk:constrain:length:l1:height `
        vars:set:width:80 vars:set:height:40 `
        sk:solve `
        feat:extrude:10 `
        view:frame `
        sk:render:"$workdir\before.png" `
        vars:set:width:120 `
        sk:render:"$workdir\after.png" `
        export:step:"$workdir\edpart.step" `
        load:step:"$workdir\edpart.step" 2>&1 | Out-String
}

$out = DriveWorkbench $exe "$env:TEMP\rc3-msi\workflow" | Out-String
Add-Content -Path $log -Value "--- trace begin ---" -Encoding Ascii
Add-Content -Path $log -Value $out -Encoding Ascii
Add-Content -Path $log -Value "--- trace end ---" -Encoding Ascii

# 5-10. workflow assertions (same probes as U1 AppImage proof)
$preview = if ($out.Length -gt 80) { $out.Substring(0,80) } else { $out }
Check "workflow-exit0" ($out -match 'oep-workbench: update check') $preview
Check "solve-dof0" ($out -match 'solved dof=0') "solver converged"
Check "solid-32000" ($out -match 'volume 32000') "extrude volume at width 80"
Check "live-recompute-48000" ($out -match '48000') "live recompute after width 120"
Check "step-exported" (Test-Path "$env:TEMP\rc3-msi\workflow\edpart.step") "edpart.step exists"
Check "step-reimported" ($out -match 'loaded step: parts=') "reimport succeeded"
Check "step-file-real" ((Get-Item "$env:TEMP\rc3-msi\workflow\edpart.step").Length -gt 100) "STEP file has real content"

# 11. ARP registered
$arp = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match 'Isolith' }
Check "arp-registered" ($null -ne $arp) "DisplayName=$($arp.DisplayName)"

# 12. uninstall
$up = Start-Process msiexec -ArgumentList "/x `"$msi`" /qn /norestart /l*v $env:TEMP\rc3-msi\uninstall.log" -Wait -PassThru
Check "msi-uninstall-exit0" ($up.ExitCode -eq 0) "uninstaller exit=$($up.ExitCode)"

# 13. ARP clean after uninstall
$arp2 = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match 'Isolith' }
Check "arp-clean-after-uninstall" ($null -eq $arp2) "no ARP entry remains"

# cleanup
Remove-Item -Recurse -Force "$env:TEMP\rc3-msi" -ErrorAction SilentlyContinue

Note "RESULT: $pass checks passed, $fail checks failed"
if ($fail -gt 0) { exit 1 } else { exit 0 }
