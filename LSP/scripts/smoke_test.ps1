$ErrorActionPreference = "Stop"
$server = Join-Path $PSScriptRoot "..\target\debug\xlang-language-server.exe"
if (-not (Test-Path $server)) {
    $server = Join-Path $PSScriptRoot "..\target\release\xlang-language-server.exe"
}

$init = '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{},"rootUri":null,"processId":null}}'
$initialized = '{"jsonrpc":"2.0","method":"initialized","params":{}}'
$open = @'
{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.x","languageId":"xlang","version":1,"text":"module main\n\ni32 add(i32 a, i32 b) {\n    return a + b;\n}\n\ni32 main() {\n    i32 x = add(40, 2);\n    return x;\n}\n"}}}
'@

function Send-LspMessage([string]$body) {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($body)
    $header = "Content-Length: $($bytes.Length)`r`n`r`n"
    [Console]::Out.Write($header)
    [Console]::Out.Flush()
    $stdout = [Console]::OpenStandardOutput()
    $stdout.Write($bytes, 0, $bytes.Length)
    $stdout.Flush()
}

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $server
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.CreateNoWindow = $true

$p = [System.Diagnostics.Process]::Start($psi)
Start-Sleep -Milliseconds 300

function Write-Raw([string]$payload) {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
    $header = "Content-Length: $($bytes.Length)`r`n`r`n"
    $p.StandardInput.Write($header)
    $p.StandardInput.Write($payload)
    $p.StandardInput.Flush()
}

Write-Raw $init
Start-Sleep -Milliseconds 500
Write-Raw $initialized
Start-Sleep -Milliseconds 200
Write-Raw $open.Trim()
Start-Sleep -Milliseconds 800

$out = ""
while ($p.StandardOutput.Peek() -ge 0) {
    $out += [char]$p.StandardOutput.Read()
}

$p.Kill()
$p.WaitForExit()

if ($out -notmatch "capabilities") { Write-Error "initialize response missing capabilities" }
if ($out -notmatch "semanticTokensProvider") { Write-Error "missing semanticTokensProvider" }
Write-Host "LSP smoke test OK"
Write-Host ($out.Substring(0, [Math]::Min(400, $out.Length)))
