# Captures the app window's own surface via PrintWindow. Unlike a desktop
# copy this needs no focus (an actively used desktop wins the focus race)
# and captures nothing outside the app. GPU-composited windows need
# PW_RENDERFULLCONTENT (2).
param([string]$ProcessName = "winui-xaml-poc", [string]$OutPath = "poc-screenshot.png")
Add-Type -AssemblyName System.Drawing
$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc -or $proc.MainWindowHandle -eq [IntPtr]::Zero) {
  throw "no window for process '$ProcessName'"
}
$sig = @'
[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
'@
$u32 = Add-Type -MemberDefinition $sig -Name Win32Window -Namespace Native -PassThru |
  Where-Object { $_.Name -eq 'Win32Window' }
$rect = New-Object Native.Win32Window+RECT
[void]$u32::GetWindowRect($proc.MainWindowHandle, [ref]$rect)
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top
if ($w -le 0 -or $h -le 0) { throw "window rect is empty" }
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
[void]$u32::PrintWindow($proc.MainWindowHandle, $hdc, 2)
$g.ReleaseHdc($hdc)
$g.Dispose()
$bmp.Save($OutPath)
