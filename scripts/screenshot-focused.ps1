# WARNING: captures the ENTIRE primary desktop, not just the app window.
# Review the output before sharing or committing it.
# Forces the named process's main window to the foreground first, because an
# actively used desktop otherwise wins the focus race.
param([string]$ProcessName = "winui-xaml-poc", [string]$OutPath = "poc-screenshot.png")
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if ($proc) {
  $sig = '[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);'
  $u32 = Add-Type -MemberDefinition $sig -Name Win32 -Namespace Native -PassThru
  $null = $u32::SetForegroundWindow($proc.MainWindowHandle)
  Start-Sleep -Milliseconds 800
}
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$bmp.Save($OutPath)
