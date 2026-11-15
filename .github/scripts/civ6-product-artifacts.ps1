# CI orchestration only. Product selection/PE/hash packaging stays in Rust.
param(
    [Parameter(Mandatory=$true)][ValidateSet('Capture','Build','FixtureCapture','FixtureInventory')][string]$Phase,
    [string]$FixtureRoot,
    [string]$FixtureProgram,
    [ValidateSet('success','failure','cancel','deadline')][string]$FixtureCase='success',
    [ValidateSet('absent','changed')][string]$FixtureProfile='absent',
    [ValidateSet('registry-extracted','directory')][string]$FixtureStorage='registry-extracted'
)
$ErrorActionPreference='Stop'
$PSNativeCommandUseErrorActionPreference=$false
$target='x86_64-pc-windows-msvc'
$root=$null
$prefix=$null
$contextPath=$null
$completionPath=$null
$binsPath=$null
$dllsPath=$null
$script:metadataIndex=0
$script:textToolIndex=0
$script:producerExit=1
function Command-Plan {
    [ordered]@{
        bins=@('cargo','+1.95.0','build','--locked','--release','--manifest-path','tools/Cargo.toml','--workspace','--bins','--message-format=json-render-diagnostics')
        dlls=@('cargo','+1.95.0','build','--locked','--release','--manifest-path','tools/Cargo.toml','--package','civ6-native-bridge-rs','--package','civ6-bootstrap-rs','--lib','--target',$target,'--message-format=json-render-diagnostics')
    }
}
function Digest-Text([string]$Text){
    $sha=[Security.Cryptography.SHA256]::Create()
    try{([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text)))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
}
function File-Fact([string]$Path,[string]$Name,[switch]$GitBlob){
    $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    $sha=[Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
    $git=$null
    try{
        $length=$f.Length
        if($length-gt536870912){throw 'build input exceeds bound'}
        if($GitBlob){
            $git=[Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA1)
            $header=[Text.Encoding]::ASCII.GetBytes('blob '+$length.ToString([Globalization.CultureInfo]::InvariantCulture)+[char]0)
            $git.AppendData($header)
        }
        $buffer=[byte[]]::new(65536);$total=0L
        while(($count=$f.Read($buffer,0,$buffer.Length))-gt0){
            $total+=$count
            if($total-gt536870912){throw 'build input changed beyond bound'}
            $sha.AppendData($buffer,0,$count)
            if($git){$git.AppendData($buffer,0,$count)}
        }
        if($total-ne$length-or$f.Length-ne$length){throw 'build input length changed'}
        $result=[ordered]@{name=$Name;bytes=$length;sha256=([BitConverter]::ToString($sha.GetHashAndReset())).Replace('-','').ToLowerInvariant()}
        if($git){$result['git_blob_sha1']=([BitConverter]::ToString($git.GetHashAndReset())).Replace('-','').ToLowerInvariant()}
        $result
    }finally{if($git){$git.Dispose()};$sha.Dispose();$f.Dispose()}
}
function Inventory-Digest($Files){
    $builder=[Text.StringBuilder]::new()
    foreach($fact in $Files){
        [void]$builder.Append([string]$fact.sha256)
        [void]$builder.Append(' ')
        [void]$builder.Append($(if($fact.git_blob_sha1){[string]$fact.git_blob_sha1}else{'-'}))
        [void]$builder.Append(' ')
        [void]$builder.Append(([ulong]$fact.bytes).ToString([Globalization.CultureInfo]::InvariantCulture))
        [void]$builder.Append(' ')
        [void]$builder.Append([string]$fact.name)
        [void]$builder.Append([char]10)
    }
    Digest-Text $builder.ToString()
}
function Package-Inventory([string]$Directory){
    $rootInfo=Get-Item -LiteralPath $Directory -Force
    if(-not$rootInfo.PSIsContainer-or($rootInfo.Attributes-band[IO.FileAttributes]::ReparsePoint)){throw 'dependency directory is not a regular namespace'}
    $folders=[Collections.Generic.Stack[string]]::new()
    $folders.Push($Directory)
    $files=@()
    while($folders.Count-gt0){
        $folder=$folders.Pop()
        foreach($entry in Get-ChildItem -LiteralPath $folder -Force){
            if($entry.Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'dependency link is outside the input contract'}
            if($entry.PSIsContainer){$folders.Push($entry.FullName);continue}
            $name=[IO.Path]::GetRelativePath($Directory,$entry.FullName).Replace('\','/')
            if([IO.Path]::IsPathRooted($name)-or$name.Split('/')-contains'..'){throw 'dependency input escapes namespace'}
            $files+=File-Fact $entry.FullName $name
            if($files.Count-gt200000){throw 'dependency input inventory exceeds bound'}
        }
    }
    if($files.Count-eq0){throw 'dependency input inventory absent'}
    $orderedNames=[string[]]@($files|ForEach-Object name)
    [Array]::Sort($orderedNames,[StringComparer]::Ordinal)
    $lookup=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach($fact in $files){$lookup.Add($fact.name,$fact)}
    foreach($name in $orderedNames){$lookup[$name]}
}
function Save-New([string]$Path,$Value){
    $b=[Text.UTF8Encoding]::new($false).GetBytes(($Value|ConvertTo-Json -Depth 100)+[Environment]::NewLine)
    $f=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try{$f.Write($b,0,$b.Length);$f.Flush($true)}finally{$f.Dispose()}
}
function Tool-Text([string]$Program,[string[]]$Arguments,[string]$Label){
    $index=$script:textToolIndex;$script:textToolIndex++
    $output=$prefix+'.'+$Phase.ToLowerInvariant()+'-tool-'+$index+'.stdout.raw'
    $result=Invoke-Native-Capture -Program $Program -Arguments $Arguments -RecordedArgv (@($Label)+$Arguments) -OutputPath $output -Label $Label -TimeoutMs 120000 -StreamLimit 16777216
    Require-Capture $result
    try{
        [Text.UTF8Encoding]::new($false,$true).GetString([IO.File]::ReadAllBytes($output))
    }catch{
        throw 'product tool text unavailable; private raw retained'
    }
}
function Git-One([string[]]$Arguments){
    $program=(Get-Command -Name git -CommandType Application -ErrorAction Stop).Source
    $value=@((Tool-Text $program $Arguments 'git') -split '\r?\n' | Where-Object { $_ -ne '' })
    if($value.Count-ne1){throw 'source identity unavailable'}
    [string]$value[0]
}
function Source-State {
    $source=Git-One -Arguments @('rev-parse','HEAD')
    $tree=Git-One -Arguments @('rev-parse','HEAD^{tree}')
    if($source-notmatch'^[0-9a-f]{40}$'-or$tree-notmatch'^[0-9a-f]{40}$'){throw 'invalid source revision'}
    $program=(Get-Command -Name git -CommandType Application -ErrorAction Stop).Source
    [void](Tool-Text $program @('diff','--quiet') 'git')
    [void](Tool-Text $program @('diff','--cached','--quiet') 'git')
    [ordered]@{source_sha=$source;source_tree=$tree;tracked_clean=$true}
}
function Profile-Facts($Environment){
    $facts=[Collections.Generic.SortedDictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach($entry in $Environment.GetEnumerator()){
        $name=([string]$entry.Key).ToUpperInvariant()
        if(-not$name.StartsWith('CARGO_PROFILE_',[StringComparison]::Ordinal)){continue}
        if($name-notmatch'^CARGO_PROFILE_[A-Z0-9_]+$'-or$facts.ContainsKey($name)){throw 'profile environment name ambiguous'}
        $facts.Add($name,[ordered]@{name=$name;present=$true;sha256=(Digest-Text ([string]$entry.Value))})
    }
    foreach($key in @('PANIC','LTO','CODEGEN_UNITS','OPT_LEVEL','DEBUG','DEBUG_ASSERTIONS','OVERFLOW_CHECKS','RPATH','SPLIT_DEBUGINFO','STRIP','INCREMENTAL','BUILD_OVERRIDE_CODEGEN_UNITS','BUILD_OVERRIDE_OPT_LEVEL','BUILD_OVERRIDE_DEBUG')){
        $name='CARGO_PROFILE_RELEASE_'+$key
        if(-not$facts.ContainsKey($name)){$facts.Add($name,[ordered]@{name=$name;present=$false;sha256=$null})}
    }
    foreach($value in $facts.Values){$value}
}
function Set-Compiler-Environment($StartInfo,$Compiler){
    $selected=[string]$Compiler.rustc_path
    if(-not[IO.Path]::IsPathFullyQualified($selected)){throw 'compiler must be the captured absolute executable'}
    $environment=$StartInfo.Environment
    $environment['RUSTC']=$selected
    $environment['CARGO_BUILD_RUSTC']=$selected
    $environment['RUSTC_WRAPPER']=''
    $environment['RUSTC_WORKSPACE_WRAPPER']=''
    $environment['CARGO_ENCODED_RUSTFLAGS']=''
    [void]$environment.Remove('RUSTFLAGS')
    [void]$environment.Remove('CARGO_BUILD_RUSTFLAGS')
    [void]$environment.Remove('CARGO_BUILD_RUSTC_WRAPPER')
    [void]$environment.Remove('CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER')
    [ordered]@{
        RUSTC=$environment['RUSTC'];CARGO_BUILD_RUSTC=$environment['CARGO_BUILD_RUSTC']
        RUSTC_WRAPPER=$environment['RUSTC_WRAPPER']
        RUSTC_WORKSPACE_WRAPPER=$environment['RUSTC_WORKSPACE_WRAPPER']
        CARGO_ENCODED_RUSTFLAGS=$environment['CARGO_ENCODED_RUSTFLAGS']
        RUSTFLAGS=$(if($environment.ContainsKey('RUSTFLAGS')){$environment['RUSTFLAGS']}else{$null})
        CARGO_BUILD_RUSTFLAGS=$(if($environment.ContainsKey('CARGO_BUILD_RUSTFLAGS')){$environment['CARGO_BUILD_RUSTFLAGS']}else{$null})
        CARGO_BUILD_RUSTC_WRAPPER=$(if($environment.ContainsKey('CARGO_BUILD_RUSTC_WRAPPER')){$environment['CARGO_BUILD_RUSTC_WRAPPER']}else{$null})
        CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER=$(if($environment.ContainsKey('CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER')){$environment['CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER']}else{$null})
    }
}
function Invoke-Native-Capture(
    [string]$Program,[string[]]$Arguments,[string[]]$RecordedArgv,
    [string]$OutputPath,[string]$Label,$Compiler=$null,
    [int]$TimeoutMs=3600000,[long]$StreamLimit=134217728,
    [long]$CancelAfterBytes=0,[string]$FixtureChildCase='',
    $ExpectedProfile=$null,[string]$FixtureProfileCase='',
    [Threading.CancellationToken]$CancellationToken=[Threading.CancellationToken]::None
){
    $native=[Diagnostics.Process]::new()
    $native.StartInfo=[Diagnostics.ProcessStartInfo]::new()
    $native.StartInfo.FileName=$Program
    $native.StartInfo.UseShellExecute=$false
    $native.StartInfo.CreateNoWindow=$true
    $native.StartInfo.WorkingDirectory=$root
    $native.StartInfo.RedirectStandardOutput=$true
    $native.StartInfo.RedirectStandardError=$true
    foreach($argument in $Arguments){$native.StartInfo.ArgumentList.Add($argument)}
    $compilerEnvironment=$null
    if($null-ne$Compiler){$compilerEnvironment=Set-Compiler-Environment $native.StartInfo $Compiler}
    if($FixtureChildCase){$native.StartInfo.Environment['CIV6_CAPTURE_FIXTURE_CASE']=$FixtureChildCase}
    if($FixtureProfileCase){
        foreach($name in @($native.StartInfo.Environment.Keys)){
            if($name.StartsWith('CARGO_PROFILE_',[StringComparison]::OrdinalIgnoreCase)){[void]$native.StartInfo.Environment.Remove($name)}
        }
        if($FixtureProfileCase-eq'changed'){
            $native.StartInfo.Environment['CARGO_PROFILE_RELEASE_PANIC']='abort'
            $native.StartInfo.Environment['CARGO_PROFILE_RELEASE_LTO']='true'
            $native.StartInfo.Environment['CARGO_PROFILE_RELEASE_CODEGEN_UNITS']='1'
            $native.StartInfo.Environment['CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS']='1'
        }
    }
    $profileEnvironment=@(Profile-Facts $native.StartInfo.Environment)
    $paths=@($OutputPath,($OutputPath+'.stderr.raw'))
    $writers=[IO.FileStream[]]::new(2);$pipes=[IO.Stream[]]::new(2);$reads=[object[]]::new(2)
    $buffers=[byte[][]]::new(2);$buffers[0]=[byte[]]::new(65536);$buffers[1]=[byte[]]::new(65536)
    $totals=[long[]]::new(2);$eof=[bool[]]::new(2)
    $started=$false;$nativePID=$null;$startedUTC=$null;$exitCode=$null;$exitObserved=$false
    $reason='start_failed';$errorRaw=$null;$killRequested=$false;$rootReaped=$false
    $cancellation=[Threading.CancellationTokenSource]::CreateLinkedTokenSource($CancellationToken)
    $deadlineCancellation=[Threading.CancellationTokenSource]::new()
    $timer=$null;$cancelTask=$null;$exitTask=$null
    try{
        for($i=0;$i-lt2;$i++){$writers[$i]=[IO.File]::Open($paths[$i],[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)}
        if($null-ne$ExpectedProfile-and($profileEnvironment|ConvertTo-Json -Depth 10 -Compress)-cne($ExpectedProfile|ConvertTo-Json -Depth 10 -Compress)){
            $reason='profile_mismatch'
            throw 'actual child release profile differs from held input'
        }
        if($cancellation.IsCancellationRequested){$reason='cancelled';throw 'native capture cancelled before start'}
        $started=$native.Start()
        if(-not$started){throw 'native child did not start'}
        $nativePID=$native.Id;$startedUTC=$native.StartTime.ToUniversalTime().ToString('o')
        $pipes=@($native.StandardOutput.BaseStream,$native.StandardError.BaseStream)
        for($i=0;$i-lt2;$i++){$reads[$i]=$pipes[$i].ReadAsync($buffers[$i],0,$buffers[$i].Length,$cancellation.Token)}
        $exitTask=$native.WaitForExitAsync()
        $timer=[Threading.Tasks.Task]::Delay($TimeoutMs,$deadlineCancellation.Token)
        $cancelTask=[Threading.Tasks.Task]::Delay(-1,$cancellation.Token)
        $reason='running'
        while(-not($eof[0]-and$eof[1]-and$exitObserved)){
            if($cancellation.IsCancellationRequested){$reason='cancelled';break}
            $pending=[Collections.Generic.List[Threading.Tasks.Task]]::new()
            for($i=0;$i-lt2;$i++){if(-not$eof[$i]){$pending.Add($reads[$i])}}
            if(-not$exitObserved){$pending.Add($exitTask)}
            $pending.Add($timer);$pending.Add($cancelTask)
            $ready=[Threading.Tasks.Task]::WhenAny($pending.ToArray()).GetAwaiter().GetResult()
            if([object]::ReferenceEquals($ready,$timer)){$reason='deadline';break}
            if([object]::ReferenceEquals($ready,$cancelTask)){$reason='cancelled';break}
            if([object]::ReferenceEquals($ready,$exitTask)){
                $exitTask.GetAwaiter().GetResult()
                $exitCode=$native.ExitCode;$exitObserved=$true
                continue
            }
            for($i=0;$i-lt2;$i++){
                if($eof[$i]-or-not[object]::ReferenceEquals($ready,$reads[$i])){continue}
                $count=$reads[$i].GetAwaiter().GetResult()
                $reads[$i]=$null
                if($count-eq0){$eof[$i]=$true;continue}
                $allowed=[int][Math]::Min([long]$count,($StreamLimit-$totals[$i]))
                if($allowed-gt0){$writers[$i].Write($buffers[$i],0,$allowed);$totals[$i]+=$allowed}
                if($allowed-ne$count){$reason=$(if($i-eq0){'stdout_limit'}else{'stderr_limit'});break}
                $reads[$i]=$pipes[$i].ReadAsync($buffers[$i],0,$buffers[$i].Length,$cancellation.Token)
            }
            if($reason-ne'running'){break}
            if($CancelAfterBytes-gt0-and($totals[0]+$totals[1])-ge$CancelAfterBytes){$reason='cancelled';$cancellation.Cancel();break}
        }
        if($eof[0]-and$eof[1]-and$exitObserved-and$reason-eq'running'){$reason='completed'}
    }catch{
        $errorRaw=$_.Exception.ToString()
        if($cancellation.IsCancellationRequested){$reason='cancelled'}elseif($reason-eq'running'){$reason='capture_error'}
    }finally{
        if($started){
            try{
                if(-not$native.HasExited){$killRequested=$true;$native.Kill($true)}
                $rootReaped=$native.WaitForExit(5000)
                if($rootReaped){$exitCode=$native.ExitCode;$exitObserved=$true}
            }catch{$errorRaw=([string]$errorRaw+[Environment]::NewLine+$_.Exception.ToString())}
        }
        $cancellation.Cancel()
        for($i=0;$i-lt2;$i++){
            # Preserve any already-delivered prefix without waiting on an open descendant pipe.
            if($null-ne$reads[$i]-and$reads[$i].Status-eq[Threading.Tasks.TaskStatus]::RanToCompletion){
                try{
                    $remaining=$reads[$i].GetAwaiter().GetResult()
                    $allowed=[int][Math]::Min([long]$remaining,($StreamLimit-$totals[$i]))
                    if($allowed-gt0){$writers[$i].Write($buffers[$i],0,$allowed);$totals[$i]+=$allowed}
                }catch{$errorRaw=([string]$errorRaw+[Environment]::NewLine+$_.Exception.ToString())}
            }
            if($null-ne$pipes[$i]){
                try{$pipes[$i].Dispose()}catch{$errorRaw=([string]$errorRaw+[Environment]::NewLine+$_.Exception.ToString())}
            }
            if($null-ne$writers[$i]){
                try{$writers[$i].Flush($true)}catch{$errorRaw=([string]$errorRaw+[Environment]::NewLine+$_.Exception.ToString())}
                try{$writers[$i].Dispose()}catch{$errorRaw=([string]$errorRaw+[Environment]::NewLine+$_.Exception.ToString())}
            }
        }
        try{$native.Dispose()}catch{$errorRaw=([string]$errorRaw+[Environment]::NewLine+$_.Exception.ToString())}
        try{$cancellation.Dispose()}catch{$errorRaw=([string]$errorRaw+[Environment]::NewLine+$_.Exception.ToString())}
        try{$deadlineCancellation.Cancel()}catch{$errorRaw=([string]$errorRaw+[Environment]::NewLine+$_.Exception.ToString())}
        $deadlineCancellation.Dispose()
        if($reason-eq'completed'-and$null-ne$errorRaw){$reason='cleanup_error'}
    }
    $stdout=File-Fact $paths[0] 'stdout'
    $stderr=File-Fact $paths[1] 'stderr'
    $complete=($reason-eq'completed'-and$eof[0]-and$eof[1]-and$exitObserved-and$null-eq$errorRaw)
    $result=[ordered]@{
        argv=$RecordedArgv;program_path=$Program;exit_code=$exitCode
        collection_complete=$complete;stop_reason=$reason
        stdout_eof=$eof[0];stderr_eof=$eof[1];root_exit_observed=$exitObserved
        native_pid=$nativePID;native_started_utc=$startedUTC;compiler_environment=$compilerEnvironment
        profile_environment=$profileEnvironment
        stdout_sha256=$stdout.sha256;stderr_sha256=$stderr.sha256
        stdout_bytes=$stdout.bytes;stderr_bytes=$stderr.bytes
        capture_error=$errorRaw;kill_requested=$killRequested;root_reaped=$rootReaped
        descendant_state='not_attested'
    }
    if($Label-in@('bins','dlls')){$result['cargo_json_sha256']=$stdout.sha256}
    Save-New ($OutputPath+'.result.json') $result
    $capturePublicEvent=[ordered]@{
        schema='civ6-native-capture/v2';phase=$Label
        exit_code=$exitCode;collection_complete=$complete;stop_reason=$reason
        stdout_bytes=$stdout.bytes;stdout_sha256=$stdout.sha256
        stderr_bytes=$stderr.bytes;stderr_sha256=$stderr.sha256
    }
    [Console]::Out.WriteLine('CIV_PRODUCT_NATIVE_CAPTURE '+($capturePublicEvent|ConvertTo-Json -Compress))
    $result
}
function Require-Capture($Result){
    if($null-ne$Result.exit_code-and$Result.exit_code-ne0){$script:producerExit=$Result.exit_code}
    if(-not$Result.collection_complete-or$Result.exit_code-ne0){throw 'product native phase did not complete successfully'}
}
function Metadata($Compiler){
    $index=$script:metadataIndex;$script:metadataIndex++
    $output=$prefix+'.'+$Phase.ToLowerInvariant()+'-metadata-'+$index+'.stdout.raw'
    $arguments=@('+1.95.0','metadata','--locked','--manifest-path','tools/Cargo.toml','--format-version','1')
    $result=Invoke-Native-Capture -Program $Compiler.cargo_dispatcher_path -Arguments $arguments -RecordedArgv (@('cargo')+$arguments) -OutputPath $output -Label 'metadata' -Compiler $Compiler -ExpectedProfile @(Profile-Facts ([Environment]::GetEnvironmentVariables())) -TimeoutMs 120000 -StreamLimit 16777216
    Require-Capture $result
    try{
        $json=[Text.UTF8Encoding]::new($false,$true).GetString([IO.File]::ReadAllBytes($output))
        $metadata=$json|ConvertFrom-Json -Depth 100 -ErrorAction Stop
        if($null-eq$metadata-or@($metadata.packages).Count-eq0){throw 'metadata packages absent'}
        $metadata
    }catch{
        Save-New ($output+'.parse-error.json') ([ordered]@{error=$_.Exception.ToString()})
        throw 'product metadata could not be decoded; private raw retained'
    }
}
function Dependency-Fact($package,$cargoHome){
    if($package.source-notin@('registry+https://github.com/rust-lang/crates.io-index','sparse+https://index.crates.io/')){
        throw 'unrecognized dependency origin'
    }
    $directory=[IO.Path]::GetFullPath((Split-Path -Parent $package.manifest_path))
    $registryRoot=[IO.Path]::GetFullPath((Join-Path $cargoHome 'registry/src'))
    $relative=[IO.Path]::GetRelativePath($registryRoot,$directory).Replace('\','/')
    $parts=$relative.Split('/')
    $normalRegistry=(-not[IO.Path]::IsPathRooted($relative)-and$parts.Count-eq2-and$parts[0]-ne'..'-and$parts[1]-eq($package.name+'-'+$package.version))
    $storage=$(if($normalRegistry){'registry-extracted'}else{'directory'})
    $inventory=@(Package-Inventory $directory)
    $byName=@{}
    foreach($fact in $inventory){$byName[$fact.name]=$fact}
    $checksumPath=Join-Path $directory '.cargo-checksum.json'
    $checksumSHA=$null
    if([IO.File]::Exists($checksumPath)){
        $checksumFact=File-Fact $checksumPath '.cargo-checksum.json'
        $checksumSHA=$checksumFact.sha256
        if($storage-eq'directory'){
            $checksum=Get-Content -LiteralPath $checksumPath -Raw|ConvertFrom-Json -Depth 100
            $entries=@($checksum.files.PSObject.Properties)
            if ($null -eq $checksum.files -or $checksum.files -isnot [pscustomobject]){throw 'directory checksum files must be an object'}
            foreach($entry in $entries){
                if([IO.Path]::IsPathRooted($entry.Name)-or$entry.Name.Split('/')-contains'..'){throw 'checksum source name escapes namespace'}
                if(-not$byName.ContainsKey($entry.Name)-or$byName[$entry.Name].sha256-ne$entry.Value){throw 'dependency bytes differ from existing directory checksum metadata'}
            }
        }
    }elseif($storage-eq'directory'){
        throw 'directory source checksum metadata absent'
    }
    $manifest=File-Fact $package.manifest_path 'Cargo.toml'
    [ordered]@{
        name=$package.name;version=$package.version;source_kind='crates.io';storage_kind=$storage
        sha256=(Inventory-Digest $inventory);files=$inventory
        manifest_sha256=$manifest.sha256;checksum_metadata_sha256=$checksumSHA
    }
}
function Input-Facts($Metadata){
    $git=(Get-Command -Name git -CommandType Application -ErrorAction Stop).Source
    $tracked=@((Tool-Text $git @('-c','core.quotepath=false','ls-files','--full-name') 'git') -split '\r?\n' | Where-Object { $_ -ne '' })
    if($tracked.Count-eq0){throw 'source file inventory absent'}
    $source=@()
    [string[]]$sourceNames=$tracked
    [Array]::Sort($sourceNames,[StringComparer]::Ordinal)
    foreach($name in $sourceNames){
        if([IO.Path]::IsPathRooted($name)-or$name.Split('/')-contains'..'){throw 'source name escapes namespace'}
        $source+=File-Fact (Join-Path $root $name) $name -GitBlob
    }
    $configs=@();$ancestor=[IO.DirectoryInfo]::new($root);$level=0
    while($ancestor){
        foreach($name in @('config','config.toml')){
            $path=Join-Path $ancestor.FullName ('.cargo/'+$name)
            if([IO.File]::Exists($path)){
                $fact=File-Fact $path $name
                $configs+=[ordered]@{scope=('ancestor-'+$level);name=$name;bytes=$fact.bytes;sha256=$fact.sha256}
            }
        }
        $ancestor=$ancestor.Parent;$level++
    }
    $cargoHome=$(if($env:CARGO_HOME){$env:CARGO_HOME}else{Join-Path $env:USERPROFILE '.cargo'})
    foreach($name in @('config','config.toml')){
        $path=Join-Path $cargoHome $name
        if([IO.File]::Exists($path)){
            $fact=File-Fact $path $name
            $configs+=[ordered]@{scope='cargo-home';name=$name;bytes=$fact.bytes;sha256=$fact.sha256}
        }
    }
    $vendors=@()
    foreach($package in @($Metadata.packages|Where-Object source|Sort-Object name,version)){
        $vendors+=Dependency-Fact $package $cargoHome
    }
    $environment=@()
    foreach($name in @('CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG')){
        $environment+=[ordered]@{name=$name;sha256=(Digest-Text ([Environment]::GetEnvironmentVariable($name)))}
    }
    $lock=File-Fact (Join-Path $root 'tools/Cargo.lock') 'tools/Cargo.lock'
    [ordered]@{
        cargo_lock_sha256=$lock.sha256
        source_inventory_sha256=(Inventory-Digest $source)
        source_file_count=$source.Count
        source_files=$source
        configs=$configs;vendors=$vendors;environment=$environment
        profile_environment=@(Profile-Facts ([Environment]::GetEnvironmentVariables()))
    }
}
function Compiler-Facts {
    foreach($name in @('RUSTC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER')){
        if([Environment]::GetEnvironmentVariable($name)){throw 'unexpected compiler override'}
    }
    $cargoDispatcher=(Get-Command -Name cargo -CommandType Application -ErrorAction Stop).Source
    $rustupDispatcher=(Get-Command -Name rustup -CommandType Application -ErrorAction Stop).Source
    $rustc=(Tool-Text $rustupDispatcher @('which','--toolchain','1.95.0','rustc') 'rustup').Trim()
    $cargo=(Tool-Text $rustupDispatcher @('which','--toolchain','1.95.0','cargo') 'rustup').Trim()
    $verbose=(Tool-Text $rustupDispatcher @('run','1.95.0','rustc','-vV') 'rustup').Trim()
    if($verbose-notmatch'(?m)^host: x86_64-pc-windows-msvc$'-or$verbose-notmatch'(?m)^release: 1.95.0$'){throw 'product compiler/host differs'}
    $toolchainRoot=Split-Path -Parent (Split-Path -Parent $rustc)
    $toolchainManifest=Join-Path $toolchainRoot 'lib/rustlib/multirust-channel-manifest.toml'
    $cargoVersion=(Tool-Text $rustupDispatcher @('run','1.95.0','cargo','--version') 'rustup').Trim()
    [ordered]@{
        toolchain='1.95.0';host=$target;rustc_verbose=$verbose;cargo_version=$cargoVersion
        rustc_sha256=(File-Fact $rustc 'rustc').sha256
        cargo_sha256=(File-Fact $cargo 'cargo').sha256
        rustup_manifest_sha256=(File-Fact $toolchainManifest 'rustup-component-manifest').sha256
        cargo_dispatcher_sha256=(File-Fact $cargoDispatcher 'cargo-dispatcher').sha256
        rustup_dispatcher_sha256=(File-Fact $rustupDispatcher 'rustup-dispatcher').sha256
        cargo_dispatcher_path=$cargoDispatcher;rustup_dispatcher_path=$rustupDispatcher
        image_os=$env:ImageOS;image_version=$env:ImageVersion
        rustc_path=$rustc;cargo_path=$cargo
    }
}
try{
if($Phase-eq'FixtureInventory'){
    if(-not$IsWindows){throw 'Windows fixture required'}
    $root=[IO.Path]::GetFullPath($FixtureRoot)
    $fixtureCargoHome=Join-Path $root 'cargo-home'
    $directory=$(if($FixtureStorage-eq'registry-extracted'){Join-Path $fixtureCargoHome 'registry/src/fixture-index/fixture-0.1.0'}else{Join-Path $root 'vendor/fixture'})
    $package=[pscustomobject]@{source='registry+https://github.com/rust-lang/crates.io-index';name='fixture';version='0.1.0';manifest_path=(Join-Path $directory 'Cargo.toml')}
    $fact=Dependency-Fact $package $fixtureCargoHome
    [Console]::Out.WriteLine('CIV_PRODUCT_INVENTORY_FIXTURE '+($fact|ConvertTo-Json -Depth 20 -Compress))
    exit 0
}
if($Phase-eq'FixtureCapture'){
    if(-not$IsWindows){throw 'Windows fixture required'}
    $root=[IO.Path]::GetFullPath($FixtureRoot)
    if([IO.Path]::GetFileName($FixtureProgram)-notmatch'^civ6_autotest_rs-[0-9a-f]+\.exe$'){throw 'own test harness required'}
    $fixtureArguments=@('--ignored','--exact','product_artifacts::tests::product_capture_fixture_child','--nocapture')
    $case=$(if($FixtureCase-in@('cancel','deadline')){'wait'}else{$FixtureCase})
    $cancelBytes=$(if($FixtureCase-eq'cancel'){131072}else{0})
    $budget=$(if($FixtureCase-eq'deadline'){1500}else{20000})
    $first=Invoke-Native-Capture -Program $FixtureProgram -Arguments $fixtureArguments -RecordedArgv @('own-test-fixture') -OutputPath (Join-Path $root 'fixture.stdout.raw') -Label 'fixture' -TimeoutMs $budget -StreamLimit 8388608 -CancelAfterBytes $cancelBytes -FixtureChildCase $case -FixtureProfileCase $FixtureProfile
    $events=@([ordered]@{exit_code=$first.exit_code;complete=$first.collection_complete;reason=$first.stop_reason;root_exit=$first.root_exit_observed;profile_environment=$first.profile_environment})
    if($FixtureCase-in@('cancel','deadline')){
        $next=Invoke-Native-Capture -Program $FixtureProgram -Arguments $fixtureArguments -RecordedArgv @('own-test-fixture') -OutputPath (Join-Path $root 'next.stdout.raw') -Label 'fixture-next' -TimeoutMs 20000 -StreamLimit 8388608 -FixtureChildCase 'success'
        $events+=[ordered]@{exit_code=$next.exit_code;complete=$next.collection_complete;reason=$next.stop_reason;root_exit=$next.root_exit_observed;profile_environment=$next.profile_environment}
    }
    [Console]::Out.WriteLine('CIV_PRODUCT_FIXTURE_RESULT '+($events|ConvertTo-Json -Depth 10 -Compress))
    exit 0
}
if(-not$IsWindows-or$env:RUNNER_ARCH-ne'X64'){throw 'Windows x64 producer required'}
$root=[IO.Path]::GetFullPath($env:GITHUB_WORKSPACE)
$prefix=Join-Path $env:RUNNER_TEMP ('civ6-product-'+$env:GITHUB_RUN_ID+'-'+$env:GITHUB_RUN_ATTEMPT)
$contextPath=$prefix+'.context.json'
$completionPath=$prefix+'.completion.json'
$binsPath=$prefix+'.bins.jsonl'
$dllsPath=$prefix+'.dlls.jsonl'
if($Phase-eq'Capture'){
    $state=Source-State
    $expected=$(if($env:CIV6_EXPECTED_SOURCE){$env:CIV6_EXPECTED_SOURCE}else{$env:GITHUB_SHA})
    if($state.source_sha-ne$expected-or$env:GITHUB_REPOSITORY-ne'AutoDevJapan/civ6-banseiki'-or$env:GITHUB_REPOSITORY_ID-ne'1392152644'){throw 'producer repository/head differs'}
    $compiler=Compiler-Facts
    $metadata=Metadata $compiler
    $inputs=Input-Facts $metadata
    $expectedCommands=Command-Plan
    $workspacePackages=@($metadata.packages|Where-Object{$null-eq$_.source}|ForEach-Object{[ordered]@{id=$_.id;name=$_.name;version=$_.version;manifest_path=$_.manifest_path}})
    $context=[ordered]@{
        schema='civ6-product-build-context/v1';repository='AutoDevJapan/civ6-banseiki';repository_id=1392152644
        source_sha=$state.source_sha;source_tree=$state.source_tree;tracked_clean=$true
        source_root=$root;target=$target;compiler=$compiler;inputs=$inputs
        workspace_packages=$workspacePackages
        commands=$expectedCommands
        producer=[ordered]@{
            run_id=[long]$env:GITHUB_RUN_ID;run_attempt=[long]$env:GITHUB_RUN_ATTEMPT
            job=$env:GITHUB_JOB;runner_os=$env:RUNNER_OS;runner_arch=$env:RUNNER_ARCH
            workflow_blob=(Git-One -Arguments @('rev-parse','HEAD:.github/workflows/rust.yml'))
        }
    }
    Save-New $contextPath $context
    Write-Output ('CIV_PRODUCT_CAPTURE source='+$state.source_sha+' tree='+$state.source_tree+' context_sha256='+(File-Fact $contextPath 'context').sha256)
    exit 0
}
$pin=[IO.File]::Open($contextPath,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try{
    $context=Get-Content -LiteralPath $contextPath -Raw|ConvertFrom-Json -Depth 100
    $contextSHA=(File-Fact $contextPath 'context').sha256
    $before=Source-State
    $currentCompiler=Compiler-Facts
    $currentInputs=Input-Facts (Metadata $currentCompiler)
    if($before.source_sha-ne$context.source_sha-or$before.source_tree-ne$context.source_tree-or($currentInputs|ConvertTo-Json -Depth 100 -Compress)-cne($context.inputs|ConvertTo-Json -Depth 100 -Compress)-or($currentCompiler|ConvertTo-Json -Depth 20 -Compress)-cne($context.compiler|ConvertTo-Json -Depth 20 -Compress)){throw 'pre-build held context no longer matches'}
    $expectedCommands=Command-Plan
    foreach($plannedPhase in @('bins','dlls')){
        if(($context.commands.$plannedPhase|ConvertTo-Json -Compress)-cne($expectedCommands[$plannedPhase]|ConvertTo-Json -Compress)){
            throw 'pre-build command differs from the fixed product recipe'
        }
    }
    $commands=[ordered]@{}
    foreach($phaseName in @('bins','dlls')){
        $command=@($context.commands.$phaseName)
        $log=$(if($phaseName-eq'bins'){$binsPath}else{$dllsPath})
        $result=Invoke-Native-Capture -Program $context.compiler.cargo_dispatcher_path -Arguments @($command[1..($command.Count-1)]) -RecordedArgv $command -OutputPath $log -Label $phaseName -Compiler $context.compiler -ExpectedProfile $context.inputs.profile_environment
        $commands[$phaseName]=$result
        if(($result.profile_environment|ConvertTo-Json -Depth 10 -Compress)-cne($context.inputs.profile_environment|ConvertTo-Json -Depth 10 -Compress)){throw 'actual child release profile differs'}
        Require-Capture $result
    }
    $after=Source-State
    $afterCompiler=Compiler-Facts
    $afterInputs=Input-Facts (Metadata $afterCompiler)
    if(($afterCompiler|ConvertTo-Json -Depth 20 -Compress)-cne($context.compiler|ConvertTo-Json -Depth 20 -Compress)){throw 'compiler input changed during build'}
    if($after.source_sha-ne$context.source_sha-or$after.source_tree-ne$context.source_tree-or($afterInputs|ConvertTo-Json -Depth 100 -Compress)-cne($context.inputs|ConvertTo-Json -Depth 100 -Compress)){throw 'build input facts changed'}
    $completion=[ordered]@{
        schema='civ6-product-build-completion/v1';source_sha=$after.source_sha;source_tree=$after.source_tree
        tracked_clean=$true;context_sha256=$contextSHA;inputs=$afterInputs;compiler=$afterCompiler;commands=$commands
    }
    Save-New $completionPath $completion
    $generator=Join-Path $root 'tools/target/release/civ6-bootstrap-scene.exe'
    $bundle=Join-Path $root 'tools/target/civ6-windows-product'
    $genArgs=@('--product-artifacts',$contextPath,$binsPath,$dllsPath,$completionPath,$bundle)
    $generated=Invoke-Native-Capture -Program $generator -Arguments $genArgs -RecordedArgv (@('civ6-bootstrap-scene')+$genArgs) -OutputPath ($prefix+'.generator.stdout.raw') -Label 'generator' -TimeoutMs 120000 -StreamLimit 16777216
    Require-Capture $generated
    Write-Output ('CIV_PRODUCT_BUNDLE source='+$after.source_sha+' tree='+$after.source_tree+' members=5')
}finally{$pin.Dispose()}
}catch{
    if($prefix){
        try{Save-New ($prefix+'.'+$Phase.ToLowerInvariant()+'.failure.json') ([ordered]@{error=$_.Exception.ToString();exit_code=$script:producerExit})}catch{}
    }
    [Console]::Out.WriteLine('CIV_PRODUCT_PHASE_FAILED exit_code='+$script:producerExit)
    exit $script:producerExit
}
