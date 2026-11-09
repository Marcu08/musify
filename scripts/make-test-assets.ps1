$ErrorActionPreference = "Stop"

$assets = Join-Path $PSScriptRoot "..\crates\audio\tests\assets"
New-Item -ItemType Directory -Force -Path $assets | Out-Null

$common = @(
    "-y", "-hide_banner", "-loglevel", "error",
    "-f", "lavfi",
    "-i", "sine=frequency=440:duration=1.2:sample_rate=44100",
    "-ac", "2",
    "-metadata", "title=Tone 440",
    "-metadata", "artist=Musify Test",
    "-metadata", "album=Audio Tests"
)

& ffmpeg @common -c:a pcm_s16le (Join-Path $assets "tone.wav")
& ffmpeg @common -c:a libmp3lame -b:a 128k (Join-Path $assets "tone.mp3")
& ffmpeg @common -c:a flac (Join-Path $assets "tone.flac")
& ffmpeg @common -c:a libvorbis -q:a 4 (Join-Path $assets "tone.ogg")
& ffmpeg @common -c:a aac -b:a 128k (Join-Path $assets "tone.m4a")

Get-ChildItem $assets | Select-Object Name, Length
