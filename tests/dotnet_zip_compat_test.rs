#[cfg(windows)]
mod windows_dotnet_zip_compat {
    use intunewin_rs::pipeline::packager::create_intunewin;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn outer_package_detection_xml_is_readable_by_dotnet_ziparchive() {
        let temp_dir = std::env::temp_dir().join(format!(
            "intunewin_dotnet_zip_compat_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("create temp dir");

        let encrypted_path = temp_dir.join("IntunePackage.intunewin");
        fs::write(&encrypted_path, b"mock encrypted content for dotnet compat")
            .expect("write mock encrypted content");

        let detection_xml = r#"<ApplicationInfo><FileName>setup.intunewin</FileName><SetupFile>setup.exe</SetupFile><UnencryptedContentSize>123</UnencryptedContentSize><EncryptionInfo>test</EncryptionInfo></ApplicationInfo>"#;
        let output_dir = temp_dir.join("output");
        let package = create_intunewin(
            Path::new(&encrypted_path),
            detection_xml,
            "setup.exe",
            Path::new(&output_dir),
        )
        .expect("create intunewin package");

        let package_str = package.to_string_lossy().replace('\'', "''");

        let ps = format!(
            "Add-Type -AssemblyName System.IO.Compression.FileSystem; \
             $zip=[System.IO.Compression.ZipFile]::OpenRead('{0}'); \
             try {{ \
               $entry=$zip.Entries | Where-Object {{ $_.FullName -eq 'IntuneWinPackage/Metadata/Detection.xml' }} | Select-Object -First 1; \
               if($null -eq $entry) {{ exit 2 }}; \
               $reader=New-Object System.IO.StreamReader($entry.Open()); \
               try {{ $content=$reader.ReadToEnd(); if([string]::IsNullOrWhiteSpace($content)) {{ exit 3 }} }} finally {{ $reader.Dispose() }} \
             }} finally {{ $zip.Dispose() }}"
            ,
            package_str
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .output()
            .expect("run powershell dotnet zip read check");

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                ".NET ZipArchive compatibility check failed\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
                output.status.code(),
                stdout,
                stderr
            );
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
