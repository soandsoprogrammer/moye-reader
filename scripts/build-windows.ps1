$ErrorActionPreference = 'Stop'

Push-Location (Split-Path -Parent $PSScriptRoot)
$stage = Join-Path ([IO.Path]::GetTempPath()) ("moye-" + [guid]::NewGuid())
try {
    cargo build --release --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Windows build failed.' }

    $app = Join-Path $stage 'MoYe'
    $fonts = Join-Path $app 'fonts'
    New-Item -ItemType Directory -Force $fonts, 'dist' | Out-Null
    $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
    Copy-Item (Join-Path $target 'x86_64-pc-windows-msvc/release/moye-reader.exe') (Join-Path $app 'MoYe.exe')
    Copy-Item README.md $app

    $upstream = 'https://raw.githubusercontent.com/notofonts/noto-cjk'
    $resources = @{
        'NotoSansCJKsc-Regular.otf' = 'Sans2.004/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf'
        'NotoSerifCJKsc-Regular.otf' = 'Serif2.003/Serif/OTF/SimplifiedChinese/NotoSerifCJKsc-Regular.otf'
        'Sans-LICENSE.txt' = 'Sans2.004/LICENSE'
        'Serif-LICENSE.txt' = 'Serif2.003/Serif/LICENSE'
    }
    foreach ($entry in $resources.GetEnumerator()) {
        Invoke-WebRequest -Uri "$upstream/$($entry.Value)" -OutFile (Join-Path $fonts $entry.Key) -MaximumRetryCount 3
    }
    Compress-Archive -Path $app -DestinationPath 'dist/MoYe-Windows-x64.zip' -Force
} finally {
    if (Test-Path $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
    Pop-Location
}
