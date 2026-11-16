# CI orchestration only. Product selection/PE/hash packaging stays in Rust.
param(
    [Parameter(Mandatory=$true)][ValidateSet('Capture','Build','FixtureCapture','FixtureInventory')][string]$Phase,
    [string]$FixtureRoot,
    [string]$FixtureProgram,
    [ValidateSet('success','failure','cancel','deadline','start_failure','stdout_open_failure','stderr_open_failure','cancel_save_failure','deadline_save_failure','limit_save_failure','save_failure','failure_save_failure','cleanup_finalize_save_failure','capture_finalize_save_failure','start_finalize_save_failure','save_partial_failure','save_flush_failure','save_dispose_failure','save_existing_file','save_existing_pending','consume_unchanged','consume_modified','consume_replaced','publish_replaced','start_publish_replaced','publish_pin_protected','post_move_read_failure','post_move_pin_failure','start_post_move_pin_failure','publication_release_failure','failure_publication_release_failure','start_publication_release_failure','cancel_publication_release_failure','deadline_publication_release_failure','limit_publication_release_failure','failure_post_emit_failure','cancel_post_emit_failure','deadline_post_emit_failure','limit_post_emit_failure','start_post_emit_failure','post_emit_failure','post_release_replacement','resolved_success','resolved_multiple','resolved_invalid','resolved_missing','working_directory_missing','public_label_git','public_label_unknown','public_label_path','cwd_bound','cwd_other')][string]$FixtureCase='success',
    [ValidateSet('absent','changed','invalid')][string]$FixtureProfile='absent',
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
function Digest-Bytes([byte[]]$Bytes){
    $sha=[Security.Cryptography.SHA256]::Create()
    try{([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
}
function Read-Verified-Stream([IO.FileStream]$Stream,$Length,[string]$SHA256,[long]$Limit=134217728){
    if($null-eq$Length-or$Length-lt0-or$Length-gt$Limit-or$SHA256-cnotmatch'^[0-9a-f]{64}$'){throw 'captured byte receipt invalid'}
    if($Stream.Length-ne$Length){throw 'captured byte length differs'}
    $Stream.Position=0
    $bytes=[byte[]]::new([int]$Length);$offset=0
    while($offset-lt$bytes.Length){
        $count=$Stream.Read($bytes,$offset,($bytes.Length-$offset))
        if($count-eq0){throw 'captured byte stream ended early'}
        $offset+=$count
    }
    if($Stream.ReadByte()-ne-1-or$Stream.Length-ne$Length-or(Digest-Bytes $bytes)-cne$SHA256){throw 'captured byte digest differs'}
    ,$bytes
}
function Read-Verified-Bytes([string]$Path,$Length,[string]$SHA256,[long]$Limit=134217728,$CaptureFailures=$null){
    if($null-eq$Length-or$Length-lt0-or$Length-gt$Limit-or$SHA256-cnotmatch'^[0-9a-f]{64}$'){throw 'captured byte receipt invalid'}
    $f=$null;$readError=$null;$disposeError=$null;$bytes=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        $bytes=Read-Verified-Stream $f $Length $SHA256 $Limit
    }catch{$readError=$_.Exception}
    finally{
        if($null-ne$f){try{$f.Dispose()}catch{$disposeError=$_.Exception}}
    }
    if($null-ne$readError){
        if($null-ne$disposeError-and$null-ne$CaptureFailures){Capture-Failure $CaptureFailures 'verified_read_dispose' $disposeError -Secondary}
        throw $readError
    }
    if($null-ne$disposeError){throw $disposeError}
    ,$bytes
}
function Save-New([string]$Path,$Value,[switch]$PassThru,[string]$FixtureFailure='',$CaptureFailures=$null){
    $b=[Text.UTF8Encoding]::new($false).GetBytes(($Value|ConvertTo-Json -Depth 100)+[Environment]::NewLine)
    if($b.Length-gt536870912){throw 'private result exceeds bound'}
    # Only the final name plus its returned digest/size is a publication receipt.
    # Failed pending files remain private, uncommitted, and are never overwritten.
    $pending=$Path+'.pending'
    $f=$null;$writeError=$null;$disposeError=$null
    try{
        $f=[IO.File]::Open($pending,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
        if($FixtureFailure-eq'partial_write'){
            $f.Write($b,0,[int][Math]::Max(1,[Math]::Floor($b.Length/2)))
            throw [IO.IOException]::new('fixture partial publication write failure')
        }
        $f.Write($b,0,$b.Length)
        if($FixtureFailure-eq'flush'){$f.Dispose()}
        $f.Flush($true)
    }catch{$writeError=$_.Exception}
    finally{
        if($null-ne$f){
            try{
                $f.Dispose()
                # A deterministic post-close fault models an unconfirmed disposal.
                if($FixtureFailure-eq'dispose'){throw [IO.IOException]::new('fixture publication dispose failure')}
            }catch{$disposeError=$_.Exception}
        }
    }
    if($null-ne$writeError){
        if($null-ne$disposeError-and$null-ne$CaptureFailures){Capture-Failure $CaptureFailures 'result_save_dispose' $disposeError -Secondary}
        throw $writeError
    }
    if($null-ne$disposeError){throw $disposeError}
    $sha=Digest-Bytes $b
    [void](Read-Verified-Bytes $pending $b.Length $sha 536870912 -CaptureFailures $CaptureFailures)
    if($FixtureFailure-eq'replace_after_verify'){
        # The explicit owned fixture synchronizes at the verified-path handoff.
        [IO.File]::Move($pending,($pending+'.original'))
        $replacement=[Text.Encoding]::UTF8.GetBytes('C:/private/publication-replacement')
        $replacementWriter=[IO.File]::Open($pending,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
        try{$replacementWriter.Write($replacement,0,$replacement.Length);$replacementWriter.Flush($true)}finally{$replacementWriter.Dispose()}
    }
    [IO.File]::Move($pending,$Path)
    $readBlocker=$null;$finalReadError=$null;$blockerReleaseError=$null
    try{
        if($FixtureFailure-eq'post_move_read'){
            $readBlocker=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::None)
        }
        $published=Read-Verified-Bytes $Path $b.Length $sha 536870912 -CaptureFailures $CaptureFailures
    }catch{$finalReadError=$_.Exception}
    finally{
        if($null-ne$readBlocker){try{$readBlocker.Dispose()}catch{$blockerReleaseError=$_.Exception}}
    }
    if($null-ne$finalReadError){
        if($null-ne$blockerReleaseError-and$null-ne$CaptureFailures){Capture-Failure $CaptureFailures 'fixture_final_read_release' $blockerReleaseError -Secondary}
        throw $finalReadError
    }
    if($null-ne$blockerReleaseError){throw $blockerReleaseError}
    if($PassThru){[ordered]@{bytes=$published.Length;sha256=(Digest-Bytes $published)}}
}
function Read-Captured-Text([string]$Path,$Result){
    $ledger=[ordered]@{first=$null;secondary=[Collections.Generic.List[object]]::new()}
    try{
        if(-not$Result.collection_complete-or$Result.stdout_state-ne'owned_hashed'){throw 'captured text is incomplete'}
        $bytes=Read-Verified-Bytes $Path $Result.stdout_bytes $Result.stdout_sha256 16777216 -CaptureFailures $ledger
        [Text.UTF8Encoding]::new($false,$true).GetString($bytes)
    }catch{
        Capture-Failure $ledger 'stdout_consume_verify' $_.Exception
        $failed=[ordered]@{schema='civ6-capture-consumer-failure/v1';collection_complete=$false;stop_reason='consumer_error';capture=$Result}
        Result-Failure $failed $ledger
        $script:lastCaptureFailure=$failed
        throw 'product captured text verification failed; private raw retained'
    }
}
function Read-Published-Capture([string]$OutputPath,$Receipt){
    if($Receipt.result_save_state-ne'saved'){throw 'capture result publication unavailable'}
    $bytes=Read-Verified-Bytes ($OutputPath+'.result.json') $Receipt.result_bytes $Receipt.result_sha256 536870912
    $record=[Text.UTF8Encoding]::new($false,$true).GetString($bytes)|ConvertFrom-Json -Depth 100 -ErrorAction Stop
    if($record.schema-ne'civ6-private-native-capture/v1'-or$record.result_save_state-ne'requires_receipt'){throw 'capture result publication schema invalid'}
    foreach($key in @('collection_complete','stop_reason','capture_stop_reason','exit_code','start_returned','root_exit_observed','failure_stage','exception_kind','exception_hresult','native_error_code','stdout_state','stdout_bytes','stdout_sha256','stderr_state','stderr_bytes','stderr_sha256')){
        if(($record.$key|ConvertTo-Json -Depth 10 -Compress)-cne($Receipt.$key|ConvertTo-Json -Depth 10 -Compress)){throw 'capture result publication differs from receipt'}
    }
    $record
}
function Public-Capture-Phase([string]$Label){
    switch -CaseSensitive ($Label){
        'git'{'git'}
        'rustup'{'rustup'}
        'cargo'{'cargo'}
        'rustc'{'rustc'}
        'metadata'{'metadata'}
        'bins'{'bins'}
        'dlls'{'dlls'}
        'generator'{'generator'}
        'fixture'{'fixture'}
        'fixture-next'{'fixture-next'}
        default{'unknown'}
    }
}
function Select-Application([object[]]$Candidates){
    $selected=$(if($Candidates.Count-gt0){$Candidates[0]}else{$null})
    $pathValue=$(if($null-ne$selected){$selected.Path}else{$null})
    $sourceValue=$(if($null-ne$selected){$selected.Source}else{$null})
    $sourceKind=$(if($null-eq$sourceValue){'none'}elseif($sourceValue-is[string]){'string'}elseif($sourceValue-is[array]){'array'}else{'other'})
    $pathString=($pathValue-is[string])
    $absolute=($pathString-and-not[string]::IsNullOrWhiteSpace($pathValue)-and[IO.Path]::IsPathFullyQualified($pathValue))
    $path=$(if($absolute){[IO.Path]::GetFullPath($pathValue)}else{''})
    $application=($null-ne$selected-and$selected.CommandType-eq[Management.Automation.CommandTypes]::Application)
    $exists=($absolute-and[IO.File]::Exists($path))
    [ordered]@{
        path=$path;valid=($application-and$absolute-and$exists)
        facts=[ordered]@{
            resolution_kind='logical_application';application_candidates=$Candidates.Count
            selected_is_application=$application;selected_path_is_string=$pathString
            selected_path_count=$(if($null-eq$pathValue){0}elseif($pathValue-is[array]){$pathValue.Count}else{1})
            selected_path_is_absolute=$absolute;selected_file_exists=$exists
            selected_source_kind=$sourceKind
            selected_source_count=$(if($null-eq$sourceValue){0}elseif($sourceValue-is[array]){$sourceValue.Count}else{1})
        }
    }
}
function Resolve-Application([string]$Name){
    $candidates=@(Get-Command -Name $Name -CommandType Application -All -ErrorAction SilentlyContinue)
    Select-Application $candidates
}
function Required-Application-Path([string]$Name){
    $resolved=Resolve-Application $Name
    if(-not$resolved.valid){throw 'required application binding unavailable'}
    [string]$resolved.path
}
function Tool-Text([string]$Program,[string[]]$Arguments,[string]$Label,[string]$ApplicationName=''){
    $index=$script:textToolIndex;$script:textToolIndex++
    $output=$prefix+'.'+$Phase.ToLowerInvariant()+'-tool-'+$index+'.stdout.raw'
    $result=Invoke-Native-Capture -Program $Program -Arguments $Arguments -RecordedArgv (@($Label)+$Arguments) -OutputPath $output -Label $Label -ApplicationName $ApplicationName -TimeoutMs 120000 -StreamLimit 16777216
    Require-Capture $result $output
    try{
        Read-Captured-Text $output $result
    }catch{
        throw 'product tool text unavailable; private raw retained'
    }
}
function Git-One([string[]]$Arguments){
    $value=@((Tool-Text -Program '' -Arguments $Arguments -Label 'git' -ApplicationName 'git') -split '\r?\n' | Where-Object { $_ -ne '' })
    if($value.Count-ne1){throw 'source identity unavailable'}
    [string]$value[0]
}
function Source-State {
    $source=Git-One -Arguments @('rev-parse','HEAD')
    $tree=Git-One -Arguments @('rev-parse','HEAD^{tree}')
    if($source-notmatch'^[0-9a-f]{40}$'-or$tree-notmatch'^[0-9a-f]{40}$'){throw 'invalid source revision'}
    [void](Tool-Text -Program '' -Arguments @('diff','--quiet') -Label 'git' -ApplicationName 'git')
    [void](Tool-Text -Program '' -Arguments @('diff','--cached','--quiet') -Label 'git' -ApplicationName 'git')
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
function Diagnostic-Codes($Exception){
    $base=$Exception.GetBaseException()
    $kind='other'
    if($base-is[ComponentModel.Win32Exception]){$kind='win32'}
    elseif($base-is[ArgumentException]){$kind='argument'}
    elseif($base-is[InvalidOperationException]){$kind='invalid_operation'}
    elseif($base-is[IO.IOException]){$kind='io'}
    elseif($base-is[OperationCanceledException]){$kind='cancelled'}
    elseif($base-is[Management.Automation.RuntimeException]){$kind='powershell'}
    [ordered]@{
        exception_kind=$kind
        exception_hresult=[int]$base.HResult
        native_error_code=$(if($base-is[ComponentModel.Win32Exception]){[int]$base.NativeErrorCode}else{$null})
    }
}
function Owned-Raw-Fact([IO.FileStream]$Stream,[string]$Name){
    $length=$Stream.Length
    if($length-gt134217728){throw 'owned raw stream exceeds bound'}
    $Stream.Position=0
    $sha=[Security.Cryptography.SHA256]::Create()
    try{$digest=([BitConverter]::ToString($sha.ComputeHash($Stream))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
    if($Stream.Length-ne$length){throw 'owned raw length changed'}
    [ordered]@{name=$Name;state='owned_hashed';bytes=$length;sha256=$digest}
}
function Capture-Failure($Ledger,[string]$Stage,$Exception,[switch]$Secondary){
    $codes=Diagnostic-Codes $Exception
    $entry=[ordered]@{
        stage=$Stage;exception_kind=$codes.exception_kind
        exception_hresult=$codes.exception_hresult;native_error_code=$codes.native_error_code
        raw=$Exception.ToString()
    }
    if(-not$Secondary-and$null-eq$Ledger.first){$Ledger.first=$entry}else{$Ledger.secondary.Add($entry)}
}
function Result-Failure($Result,$Ledger){
    $first=$Ledger.first
    $Result['failure_stage']=$(if($null-ne$first){$first.stage}else{$null})
    $Result['exception_kind']=$(if($null-ne$first){$first.exception_kind}else{$null})
    $Result['exception_hresult']=$(if($null-ne$first){$first.exception_hresult}else{$null})
    $Result['native_error_code']=$(if($null-ne$first){$first.native_error_code}else{$null})
    $Result['capture_error']=$(if($null-ne$first){$first.raw}else{$null})
    $Result['secondary_errors']=@($Ledger.secondary.ToArray())
}
function Test-Capture-Primary-Outcome($Result){
    ($Result.stop_reason-ne'completed'-or$null-ne$Result.failure_stage-or($null-ne$Result.exit_code-and$Result.exit_code-ne0))
}
function Invalidate-Capture-Publication($Result,$Failures){
    $hadPrimary=Test-Capture-Primary-Outcome $Result
    $Result.collection_complete=$false
    if(-not$hadPrimary){$Result.stop_reason='publication_finalize_failed'}
    $Result.result_save_state='unavailable'
    $Result.result_bytes=$null;$Result.result_sha256=$null
    Result-Failure $Result $Failures
    $script:lastCaptureFailure=$Result
}
function Invoke-Native-Capture(
    [string]$Program,[string[]]$Arguments,[string[]]$RecordedArgv,
    [string]$OutputPath,[string]$Label,$Compiler=$null,
    [int]$TimeoutMs=3600000,[long]$StreamLimit=134217728,
    [long]$CancelAfterBytes=0,[string]$FixtureChildCase='',
    $ExpectedProfile=$null,[string]$FixtureProfileCase='',
    [Threading.CancellationToken]$CancellationToken=[Threading.CancellationToken]::None,
    [switch]$FixtureCleanupFault,[switch]$FixtureFinalizeFault,
    [switch]$MatchCurrentProfile,[string]$FixtureResultSaveCase='',
    [string]$ApplicationName='',[string]$FixtureResolutionCase=''
){
    $failures=[ordered]@{first=$null;secondary=[Collections.Generic.List[object]]::new()}
    $native=$null;$cancellation=$null;$deadlineCancellation=$null
    $compilerEnvironment=$null;$profileEnvironment=$null
    $bindingFacts=$null;$boundWorkingDirectory=$null
    $publicPhase=Public-Capture-Phase $Label
    $writers=[IO.FileStream[]]::new(2);$pipes=[IO.Stream[]]::new(2);$reads=[object[]]::new(2)
    $created=[bool[]]::new(2)
    $facts=@(
        [ordered]@{name='stdout';state='not_owned';bytes=$null;sha256=$null},
        [ordered]@{name='stderr';state='not_owned';bytes=$null;sha256=$null}
    )
    $totals=[long[]]::new(2);$eof=[bool[]]::new(2)
    $started=$false;$nativePID=$null;$startedUTC=$null;$exitCode=$null;$exitObserved=$false
    $reason='start_failed';$killRequested=$false;$rootReaped=$false
    $stage='initialize';$startReturned=$null;$captureReason=$null
    $timer=$null;$cancelTask=$null;$exitTask=$null
    $script:lastCaptureFailure=$null
    try{
        $stage='process_create';$native=[Diagnostics.Process]::new()
        $stage='start_info';$native.StartInfo=[Diagnostics.ProcessStartInfo]::new()
        $stage='program_binding'
        $resolution=$null
        if($ApplicationName){$resolution=Resolve-Application $ApplicationName}
        if($FixtureResolutionCase-in@('multiple','invalid','missing')){
            $candidates=@()
            if($FixtureResolutionCase-ne'missing'){
                $pathValue=$(if($FixtureResolutionCase-eq'invalid'){@($Program,$Program)}else{$Program})
                $candidates=@(
                    [pscustomobject]@{CommandType=[Management.Automation.CommandTypes]::Application;Path=$pathValue;Source=@('fixture-source-a','fixture-source-b')},
                    [pscustomobject]@{CommandType=[Management.Automation.CommandTypes]::Application;Path=$Program;Source='fixture-source'}
                )
            }
            $resolution=Select-Application $candidates
            $resolution.facts.resolution_kind='fixture_descriptors'
        }
        if($null-ne$resolution){
            $Program=[string]$resolution.path
            $bindingFacts=$resolution.facts
        }else{
            $bindingFacts=[ordered]@{
                resolution_kind='direct_path';application_candidates=0;selected_is_application=$false
                selected_path_is_string=($Program-is[string]);selected_path_count=1
                selected_path_is_absolute=([IO.Path]::IsPathFullyQualified($Program))
                selected_file_exists=([IO.File]::Exists($Program))
                selected_source_kind='not_applicable';selected_source_count=0
            }
        }
        $native.StartInfo.FileName=$Program
        $native.StartInfo.UseShellExecute=$false
        $native.StartInfo.CreateNoWindow=$true
        $rootString=($script:root-is[string]-and-not[string]::IsNullOrWhiteSpace($script:root))
        $rootAbsolute=($rootString-and[IO.Path]::IsPathFullyQualified($script:root))
        $boundWorkingDirectory=$(if($rootAbsolute){[IO.Path]::GetFullPath($script:root)}else{''})
        if($FixtureResolutionCase-eq'missing_cwd'){$boundWorkingDirectory=Join-Path $script:root 'civ6-fixture-missing-cwd'}
        if($FixtureResolutionCase-eq'other_cwd'){$boundWorkingDirectory=Join-Path $script:root 'fixture-other-cwd'}
        $native.StartInfo.WorkingDirectory=$boundWorkingDirectory
        $native.StartInfo.RedirectStandardOutput=$true
        $native.StartInfo.RedirectStandardError=$true
        $bindingEvent=[ordered]@{schema='civ6-process-binding/v1';phase=$publicPhase}
        foreach($key in $bindingFacts.Keys){$bindingEvent[$key]=$bindingFacts[$key]}
        $bindingEvent['filename_matches_selection']=($native.StartInfo.FileName-ceq$Program)
        $bindingEvent['argument_count']=$Arguments.Count
        $bindingEvent['working_directory_is_string']=$rootString
        $bindingEvent['working_directory_is_absolute']=$rootAbsolute
        $bindingEvent['working_directory_exists']=[IO.Directory]::Exists($boundWorkingDirectory)
        [Console]::Out.WriteLine('CIV_PRODUCT_PROCESS_BINDING '+($bindingEvent|ConvertTo-Json -Compress))
        if($null-ne$resolution-and-not$resolution.valid){throw 'application binding unavailable'}
        $stage='working_directory'
        if(-not$rootAbsolute-or-not[IO.Directory]::Exists($boundWorkingDirectory)){throw 'native working directory unavailable'}
        $stage='arguments'
        foreach($argument in $Arguments){$native.StartInfo.ArgumentList.Add($argument)}
        $stage='compiler_environment'
        if($null-ne$Compiler){$compilerEnvironment=Set-Compiler-Environment $native.StartInfo $Compiler}
        $stage='child_environment'
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
            if($FixtureProfileCase-eq'invalid'){$native.StartInfo.Environment['CARGO_PROFILE_RELEASE_BAD-NAME']='fixture'}
        }
        $stage='profile_environment';$profileEnvironment=@(Profile-Facts $native.StartInfo.Environment)
        if($MatchCurrentProfile){$stage='expected_profile';$ExpectedProfile=@(Profile-Facts ([Environment]::GetEnvironmentVariables()))}
        $stage='buffers'
        $buffers=[byte[][]]::new(2);$buffers[0]=[byte[]]::new(65536);$buffers[1]=[byte[]]::new(65536)
        $paths=@($OutputPath,($OutputPath+'.stderr.raw'))
        $stage='read_cancellation';$cancellation=[Threading.CancellationTokenSource]::CreateLinkedTokenSource($CancellationToken)
        $stage='deadline_cancellation';$deadlineCancellation=[Threading.CancellationTokenSource]::new()
        for($i=0;$i-lt2;$i++){$stage=$(if($i-eq0){'stdout_open'}else{'stderr_open'});$writers[$i]=[IO.File]::Open($paths[$i],[IO.FileMode]::CreateNew,[IO.FileAccess]::ReadWrite,[IO.FileShare]::Read);$created[$i]=$true}
        $stage='profile_check'
        if($null-ne$ExpectedProfile-and($profileEnvironment|ConvertTo-Json -Depth 10 -Compress)-cne($ExpectedProfile|ConvertTo-Json -Depth 10 -Compress)){
            $reason='profile_mismatch'
            throw 'actual child release profile differs from held input'
        }
        if($cancellation.IsCancellationRequested){$reason='cancelled';throw 'native capture cancelled before start'}
        $stage='process_start'
        $started=$native.Start();$startReturned=$started
        if(-not$started){throw 'native child did not start'}
        $stage='pid_read';$nativePID=$native.Id
        $stage='start_time_read';$startedUTC=$native.StartTime.ToUniversalTime().ToString('o')
        $stage='pipe_bind'
        $pipes=@($native.StandardOutput.BaseStream,$native.StandardError.BaseStream)
        for($i=0;$i-lt2;$i++){$stage=$(if($i-eq0){'stdout_read_begin'}else{'stderr_read_begin'});$reads[$i]=$pipes[$i].ReadAsync($buffers[$i],0,$buffers[$i].Length,$cancellation.Token)}
        $stage='exit_task_begin';$exitTask=$native.WaitForExitAsync()
        $stage='deadline_task_begin';$timer=[Threading.Tasks.Task]::Delay($TimeoutMs,$deadlineCancellation.Token)
        $stage='cancel_task_begin';$cancelTask=[Threading.Tasks.Task]::Delay(-1,$cancellation.Token)
        $reason='running'
        while(-not($eof[0]-and$eof[1]-and$exitObserved)){
            if($cancellation.IsCancellationRequested){$reason='cancelled';break}
            $pending=[Collections.Generic.List[Threading.Tasks.Task]]::new()
            for($i=0;$i-lt2;$i++){if(-not$eof[$i]){$pending.Add($reads[$i])}}
            if(-not$exitObserved){$pending.Add($exitTask)}
            $pending.Add($timer);$pending.Add($cancelTask)
            $stage='drain_wait'
            $ready=[Threading.Tasks.Task]::WhenAny($pending.ToArray()).GetAwaiter().GetResult()
            if([object]::ReferenceEquals($ready,$timer)){$reason='deadline';break}
            if([object]::ReferenceEquals($ready,$cancelTask)){$reason='cancelled';break}
            if([object]::ReferenceEquals($ready,$exitTask)){
                # The runtime Task subtype can return VoidTaskResult to PowerShell.
                # Observe completion and exceptions without adding a receipt-stream item.
                $stage='exit_observe';[void]$exitTask.GetAwaiter().GetResult()
                $exitCode=$native.ExitCode;$exitObserved=$true
                continue
            }
            for($i=0;$i-lt2;$i++){
                if($eof[$i]-or-not[object]::ReferenceEquals($ready,$reads[$i])){continue}
                $stage=$(if($i-eq0){'stdout_read_end'}else{'stderr_read_end'})
                $count=$reads[$i].GetAwaiter().GetResult()
                $reads[$i]=$null
                if($count-eq0){$eof[$i]=$true;continue}
                $allowed=[int][Math]::Min([long]$count,($StreamLimit-$totals[$i]))
                $stage=$(if($i-eq0){'stdout_write'}else{'stderr_write'})
                if($allowed-gt0){$writers[$i].Write($buffers[$i],0,$allowed);$totals[$i]+=$allowed}
                if($allowed-ne$count){$reason=$(if($i-eq0){'stdout_limit'}else{'stderr_limit'});break}
                $stage=$(if($i-eq0){'stdout_read_begin'}else{'stderr_read_begin'})
                $reads[$i]=$pipes[$i].ReadAsync($buffers[$i],0,$buffers[$i].Length,$cancellation.Token)
            }
            if($reason-ne'running'){break}
            if($CancelAfterBytes-gt0-and($totals[0]+$totals[1])-ge$CancelAfterBytes){$reason='cancelled';$cancellation.Cancel();break}
        }
        if($eof[0]-and$eof[1]-and$exitObserved-and$reason-eq'running'){$reason='completed'}

    }catch{
        Capture-Failure $failures $stage $_.Exception
        if($reason-eq'running'){$reason=$(if($null-ne$cancellation-and$cancellation.IsCancellationRequested){'cancelled'}else{'capture_error'})}
    }finally{
        $captureReason=$reason
        if(($FixtureCleanupFault-or$FixtureFinalizeFault)-and$created[0]){
            try{$writers[0].Dispose()}catch{Capture-Failure $failures 'fixture_raw_prepare' $_.Exception}
        }
        if($started){
            $cleanupStage='root_status'
            try{
                if(-not$native.HasExited){$cleanupStage='root_kill';$killRequested=$true;$native.Kill($true)}
                $cleanupStage='root_wait'
                $reapBudget=$(if($FixtureCleanupFault){-2}else{5000})
                $rootReaped=$native.WaitForExit($reapBudget)
                if($rootReaped){$cleanupStage='root_exit_code';$exitCode=$native.ExitCode;$exitObserved=$true}
            }catch{Capture-Failure $failures $cleanupStage $_.Exception}
        }
        if($null-ne$cancellation){try{$cancellation.Cancel()}catch{Capture-Failure $failures 'read_cancel' $_.Exception}}
        for($i=0;$i-lt2;$i++){
            $streamName=$(if($i-eq0){'stdout'}else{'stderr'})
            if($null-ne$reads[$i]-and$reads[$i].Status-eq[Threading.Tasks.TaskStatus]::RanToCompletion){
                $cleanupStage=$streamName+'_pending_result'
                try{
                    $remaining=$reads[$i].GetAwaiter().GetResult()
                    $allowed=[int][Math]::Min([long]$remaining,($StreamLimit-$totals[$i]))
                    $cleanupStage=$streamName+'_pending_write'
                    if($allowed-gt0){$writers[$i].Write($buffers[$i],0,$allowed);$totals[$i]+=$allowed}
                }catch{Capture-Failure $failures $cleanupStage $_.Exception}
            }
            if($null-ne$pipes[$i]){
                try{$pipes[$i].Dispose()}catch{Capture-Failure $failures ($streamName+'_pipe_dispose') $_.Exception}
            }
            if($created[$i]){
                $facts[$i]=[ordered]@{name=$streamName;state='owned_unavailable';bytes=$null;sha256=$null}
                $cleanupStage=$streamName+'_flush'
                try{
                    $writers[$i].Flush($true)
                    $cleanupStage=$streamName+'_hash'
                    $facts[$i]=Owned-Raw-Fact $writers[$i] $streamName
                }catch{Capture-Failure $failures $cleanupStage $_.Exception}
                finally{
                    try{$writers[$i].Dispose()}catch{Capture-Failure $failures ($streamName+'_raw_dispose') $_.Exception}
                }
            }
        }
        if($null-ne$native){try{$native.Dispose()}catch{Capture-Failure $failures 'process_dispose' $_.Exception}}
        if($null-ne$cancellation){try{$cancellation.Dispose()}catch{Capture-Failure $failures 'read_cancel_dispose' $_.Exception}}
        if($null-ne$deadlineCancellation){
            try{$deadlineCancellation.Cancel()}catch{Capture-Failure $failures 'deadline_cancel' $_.Exception}
            try{$deadlineCancellation.Dispose()}catch{Capture-Failure $failures 'deadline_dispose' $_.Exception}
        }
        if($reason-eq'completed'-and$null-ne$failures.first){$reason='cleanup_error'}
    }
    $stdout=$facts[0]
    $stderr=$facts[1]
    $complete=($reason-eq'completed'-and$eof[0]-and$eof[1]-and$exitObserved-and$null-eq$failures.first-and$stdout.state-eq'owned_hashed'-and$stderr.state-eq'owned_hashed')
    # The file is a candidate snapshot; success requires the later returned receipt.
    $result=[ordered]@{
        schema='civ6-private-native-capture/v1';result_save_state='requires_receipt'
        argv=$RecordedArgv;program_path=$Program;working_directory=$boundWorkingDirectory;binding_facts=$bindingFacts;exit_code=$exitCode
        collection_complete=$complete;stop_reason=$reason
        capture_stop_reason=$captureReason;start_returned=$startReturned
        failure_stage=$null;exception_kind=$null;exception_hresult=$null;native_error_code=$null
        stdout_eof=$eof[0];stderr_eof=$eof[1];root_exit_observed=$exitObserved
        native_pid=$nativePID;native_started_utc=$startedUTC;compiler_environment=$compilerEnvironment
        profile_environment=$profileEnvironment
        stdout_state=$stdout.state;stderr_state=$stderr.state
        stdout_sha256=$stdout.sha256;stderr_sha256=$stderr.sha256
        stdout_bytes=$stdout.bytes;stderr_bytes=$stderr.bytes
        capture_error=$null;secondary_errors=@();kill_requested=$killRequested;root_reaped=$rootReaped
        descendant_state='not_attested'
    }
    Result-Failure $result $failures
    if($Label-in@('bins','dlls')){$result['cargo_json_sha256']=$stdout.sha256}
    $resultSaveState='saved'
    $resultSaveError=$null;$publication=$null;$publicationPin=$null;$publicationBlocker=$null
    $publicationStage='result_save';$publicationPinChecked=$false;$fixtureLateExpected=$null
    try{
        $publication=Save-New ($OutputPath+'.result.json') $result -PassThru -FixtureFailure $FixtureResultSaveCase -CaptureFailures $failures
        $publicationStage='result_publish_open'
        if($FixtureResultSaveCase-eq'post_move_pin'){
            $publicationBlocker=[IO.File]::Open(($OutputPath+'.result.json'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::None)
        }
        $publicationPin=[IO.File]::Open(($OutputPath+'.result.json'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        $publicationStage='result_publish_verify'
        $publishedBytes=Read-Verified-Stream $publicationPin $publication.bytes $publication.sha256 536870912
        $publication.bytes=$publishedBytes.Length
        $publication.sha256=Digest-Bytes $publishedBytes
        if($FixtureResultSaveCase-eq'pin_check'){
            $publicationStage='fixture_publish_pin'
            $rejected=$false
            try{[IO.File]::Move(($OutputPath+'.result.json'),($OutputPath+'.result.json.unexpected'))}
            catch [IO.IOException]{
                if(($_.Exception.GetBaseException().HResult-band65535)-ne32){throw}
                $rejected=$true
            }
            if(-not$rejected){throw 'publication read pin did not reject rename'}
            $publicationPinChecked=$true
        }
    }catch{
        $resultSaveState='unavailable'
        $hadPrimary=Test-Capture-Primary-Outcome $result
        Capture-Failure $failures $publicationStage $_.Exception -Secondary:$hadPrimary
        $publication=$null
        $resultSaveError=$(if($hadPrimary){$failures.secondary[$failures.secondary.Count-1]}else{$failures.first})
        if(-not$hadPrimary){$reason='result_save_failed'}
        $complete=$false
        $result.collection_complete=$complete
        $result.stop_reason=$reason
        Result-Failure $result $failures
    }
    $result['result_save_state']=$resultSaveState
    $result['result_save_error']=$resultSaveError
    if($FixtureResultSaveCase-eq'pin_check'){$result['fixture_publication_pin_checked']=$publicationPinChecked}
    $result['result_bytes']=$(if($null-ne$publication){$publication.bytes}else{$null})
    $result['result_sha256']=$(if($null-ne$publication){$publication.sha256}else{$null})
    if(-not$result.collection_complete-or$result.exit_code-ne0){$script:lastCaptureFailure=$result}
    # Finish every known owned cleanup before taking the advisory snapshot.
    $publicationCleanupFailed=$false
    if($null-ne$publicationPin){
        try{
            $publicationPin.Dispose()
            if($FixtureResultSaveCase-eq'release_failure'){throw [IO.IOException]::new('fixture publication release failure')}
        }catch{
            if($FixtureResultSaveCase-eq'release_failure'){
                $fixtureLateBase=$_.Exception.GetBaseException()
                $fixtureLateExpected=[ordered]@{
                    stage='result_publication_release';exception_kind='io'
                    exception_hresult=[int]$fixtureLateBase.HResult;native_error_code=$null;raw_sha256=(Digest-Text ($_.Exception.ToString()))
                    exception_matches=($fixtureLateBase-is[IO.IOException]-and$fixtureLateBase.Message-ceq'fixture publication release failure')
                }
            }
            Capture-Failure $failures 'result_publication_release' $_.Exception -Secondary:(Test-Capture-Primary-Outcome $result)
            $publicationCleanupFailed=$true
        }
    }
    if($null-ne$publicationBlocker){
        try{$publicationBlocker.Dispose()}catch{
            Capture-Failure $failures 'fixture_publication_blocker_release' $_.Exception -Secondary:(Test-Capture-Primary-Outcome $result)
            $publicationCleanupFailed=$true
        }
    }
    if($publicationCleanupFailed){Invalidate-Capture-Publication $result $failures}
    if($FixtureResultSaveCase-eq'replace_after_release'){
        try{
            [IO.File]::Move(($OutputPath+'.result.json'),($OutputPath+'.result.json.after-release.original'))
            Save-New ($OutputPath+'.result.json') ([ordered]@{fixture='post_release_replacement'})
        }catch{
            Capture-Failure $failures 'fixture_post_release_replace' $_.Exception -Secondary:(Test-Capture-Primary-Outcome $result)
            Invalidate-Capture-Publication $result $failures
        }
    }
    # This is historical data, never a standalone producer-success receipt.
    # Callers require the actual return and a newly verified consumer buffer.
    # The workflow also requires producer exit success; no live path is promised.
    $capturePublicEvent=[ordered]@{
        schema='civ6-native-capture/v3';evidence_kind='historical_capture_snapshot';phase=$publicPhase
        native_exit_code=$result.exit_code;collection_complete=$result.collection_complete;stop_reason=$result.stop_reason
        capture_stop_reason=$result.capture_stop_reason;start_returned=$result.start_returned;root_exit_observed=$result.root_exit_observed
        failure_stage=$result.failure_stage
        exception_kind=$result.exception_kind;exception_hresult=$result.exception_hresult;native_error_code=$result.native_error_code
        result_save_state=$result.result_save_state;result_bytes=$result.result_bytes;result_sha256=$result.result_sha256;secondary_failure_count=$result.secondary_errors.Count
        stdout_state=$result.stdout_state;stderr_state=$result.stderr_state
        stdout_bytes=$result.stdout_bytes;stdout_sha256=$result.stdout_sha256
        stderr_bytes=$result.stderr_bytes;stderr_sha256=$result.stderr_sha256
    }
    try{
        [Console]::Out.WriteLine('CIV_PRODUCT_NATIVE_CAPTURE '+($capturePublicEvent|ConvertTo-Json -Compress))
        if($FixtureResultSaveCase-eq'emit_failure'){throw [IO.IOException]::new('fixture post-emission write failure')}
    }catch{
        if($FixtureResultSaveCase-eq'emit_failure'){
            $fixtureLateBase=$_.Exception.GetBaseException()
            $fixtureLateExpected=[ordered]@{
                stage='public_event_write';exception_kind='io'
                exception_hresult=[int]$fixtureLateBase.HResult;native_error_code=$null;raw_sha256=(Digest-Text ($_.Exception.ToString()))
                exception_matches=($fixtureLateBase-is[IO.IOException]-and$fixtureLateBase.Message-ceq'fixture post-emission write failure')
            }
        }
        Capture-Failure $failures 'public_event_write' $_.Exception -Secondary:(Test-Capture-Primary-Outcome $result)
        Invalidate-Capture-Publication $result $failures
    }
    if($null-ne$fixtureLateExpected){$result['fixture_late_expected']=$fixtureLateExpected}
    $result
}
function Require-Capture($Result,[string]$OutputPath){
    if($null-ne$Result.exit_code-and$Result.exit_code-ne0){$script:producerExit=$Result.exit_code}
    if(-not$Result.collection_complete-or$Result.exit_code-ne0-or$Result.result_save_state-ne'saved'-or[string]::IsNullOrWhiteSpace($OutputPath)){
        $script:lastCaptureFailure=$Result
        throw 'product native phase did not complete with a current publication receipt'
    }
    try{[void](Read-Published-Capture $OutputPath $Result)}
    catch{
        $ledger=[ordered]@{first=$null;secondary=[Collections.Generic.List[object]]::new()}
        Capture-Failure $ledger 'result_consume_verify' $_.Exception
        $failed=[ordered]@{schema='civ6-capture-consumer-failure/v1';collection_complete=$false;stop_reason='consumer_error';capture=$Result}
        Result-Failure $failed $ledger
        $script:lastCaptureFailure=$failed
        throw 'product native publication receipt could not be verified'
    }
}
function Metadata($Compiler){
    $index=$script:metadataIndex;$script:metadataIndex++
    $output=$prefix+'.'+$Phase.ToLowerInvariant()+'-metadata-'+$index+'.stdout.raw'
    $arguments=@('+1.95.0','metadata','--locked','--manifest-path','tools/Cargo.toml','--format-version','1')
    $result=Invoke-Native-Capture -Program $Compiler.cargo_dispatcher_path -Arguments $arguments -RecordedArgv (@('cargo')+$arguments) -OutputPath $output -Label 'metadata' -Compiler $Compiler -MatchCurrentProfile -TimeoutMs 120000 -StreamLimit 16777216
    Require-Capture $result $output
    try{
        $json=Read-Captured-Text $output $result
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
    $tracked=@((Tool-Text -Program '' -Arguments @('-c','core.quotepath=false','ls-files','--full-name') -Label 'git' -ApplicationName 'git') -split '\r?\n' | Where-Object { $_ -ne '' })
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
    $cargoDispatcher=Required-Application-Path 'cargo'
    $rustupDispatcher=Required-Application-Path 'rustup'
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
function Fixture-Late-Diagnostic-Matches($Actual,$Expected){
    if($Actual-isnot[Collections.IDictionary]-or$null-eq$Expected-or-not$Expected.exception_matches){return $false}
    foreach($key in @('stage','exception_kind','exception_hresult','native_error_code','raw')){
        if(-not$Actual.Contains($key)){return $false}
    }
    ($Actual.stage-is[string]-and$Actual.stage-ceq$Expected.stage-and
     $Actual.exception_kind-is[string]-and$Actual.exception_kind-ceq$Expected.exception_kind-and
     $Actual.exception_hresult-is[int]-and$Actual.exception_hresult-eq$Expected.exception_hresult-and
     $null-eq$Actual.native_error_code-and$Actual.raw-is[string]-and
     -not[string]::IsNullOrWhiteSpace($Actual.raw)-and(Digest-Text $Actual.raw)-ceq$Expected.raw_sha256)
}
function Fixture-Capture-Summary([string]$OutputPath,$Result){
    $returnedSnapshot=[ordered]@{
        exit_code=$Result.exit_code;complete=$Result.collection_complete;reason=$Result.stop_reason
        root_exit=$Result.root_exit_observed;profile_environment=$Result.profile_environment
        failure_stage=$Result.failure_stage;start_returned=$Result.start_returned
        exception_kind=$Result.exception_kind;exception_hresult=$Result.exception_hresult;native_error_code=$Result.native_error_code
        stdout_state=$Result.stdout_state;stderr_state=$Result.stderr_state
        stdout_bytes=$Result.stdout_bytes;stderr_bytes=$Result.stderr_bytes
        stdout_sha256=$Result.stdout_sha256;stderr_sha256=$Result.stderr_sha256
        capture_stop_reason=$Result.capture_stop_reason;result_save_state=$Result.result_save_state
        result_bytes=$Result.result_bytes;result_sha256=$Result.result_sha256
        secondary_failure_count=$Result.secondary_errors.Count
        secondary_stages=@($Result.secondary_errors|ForEach-Object stage)
        publication_pin_checked=$Result.fixture_publication_pin_checked
    }
    $lateVerified=$null;$lateSecondary=$null;$lateRejected=0
    $lateExpected=$Result.fixture_late_expected
    if($null-ne$lateExpected){
        $lateSecondary=$script:FixtureCase-in@(
            'failure_publication_release_failure','start_publication_release_failure',
            'cancel_publication_release_failure','deadline_publication_release_failure','limit_publication_release_failure',
            'failure_post_emit_failure','start_post_emit_failure','cancel_post_emit_failure','deadline_post_emit_failure','limit_post_emit_failure'
        )
        $lateActual=$null
        if($lateSecondary){
            $lateEntries=@($Result.secondary_errors|Where-Object{$_.stage-ceq$lateExpected.stage})
            if($lateEntries.Count-eq1){$lateActual=$lateEntries[0]}
        }else{
            $lateActual=[ordered]@{
                stage=$Result.failure_stage;exception_kind=$Result.exception_kind;exception_hresult=$Result.exception_hresult
                native_error_code=$Result.native_error_code;raw=$Result.capture_error
            }
        }
        $lateVerified=Fixture-Late-Diagnostic-Matches $lateActual $lateExpected
        if($lateActual-is[Collections.IDictionary]){
            $lateWrong=[ordered]@{
                stage='fixture_mismatch';exception_kind='other';exception_hresult=($lateExpected.exception_hresult-bxor1)
                native_error_code=0;raw='fixture private diagnostic mismatch'
            }
            foreach($changedKey in $lateWrong.Keys){
                foreach($removeField in @($true,$false)){
                    $lateCopy=[ordered]@{}
                    foreach($key in $lateActual.Keys){$lateCopy[$key]=$lateActual[$key]}
                    if($removeField){[void]$lateCopy.Remove($changedKey)}else{$lateCopy[$changedKey]=$lateWrong[$changedKey]}
                    if(-not(Fixture-Late-Diagnostic-Matches $lateCopy $lateExpected)){$lateRejected++}
                }
            }
        }
    }
    $returnedSnapshot['late_diagnostic_verified']=$lateVerified
    $returnedSnapshot['late_diagnostic_secondary']=$lateSecondary
    $returnedSnapshot['late_diagnostic_negative_rejections']=$lateRejected
    $verified=$false
    try{[void](Read-Published-Capture $OutputPath $Result);$verified=$true}catch{}
    $savedLast=$script:lastCaptureFailure;$savedExit=$script:producerExit
    $consumerAccepted=$false;$candidateAccepted=$false;$candidateReaderAccepted=$false
    $receiptlessAccepted=$false;$resourcesReleased=$null
    try{
        try{Require-Capture $Result $OutputPath;$consumerAccepted=$true}catch{}
        $receiptless=$Result|ConvertTo-Json -Depth 100|ConvertFrom-Json -Depth 100
        [void]$receiptless.PSObject.Properties.Remove('result_bytes')
        [void]$receiptless.PSObject.Properties.Remove('result_sha256')
        try{Require-Capture $receiptless $OutputPath;$receiptlessAccepted=$true}catch{}
        if([IO.File]::Exists($OutputPath+'.result.json')){
            try{
                # This unverified candidate is used only as a negative consumer input.
                $candidate=[Text.UTF8Encoding]::new($false,$true).GetString([IO.File]::ReadAllBytes($OutputPath+'.result.json'))|ConvertFrom-Json -Depth 100
                try{Require-Capture $candidate $OutputPath;$candidateAccepted=$true}catch{}
                try{[void](Read-Published-Capture $OutputPath $candidate);$candidateReaderAccepted=$true}catch{}
            }catch{}
            $probe=$null
            try{
                $probe=[IO.File]::Open(($OutputPath+'.result.json'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::None)
                $resourcesReleased=$true
            }catch{$resourcesReleased=$false}
            finally{if($null-ne$probe){$probe.Dispose()}}
        }
    }finally{$script:lastCaptureFailure=$savedLast;$script:producerExit=$savedExit}
    $probeFields=[ordered]@{
        publication_verified=$verified;last_failure_stage=$script:lastCaptureFailure.failure_stage
        actual_consumer_accepted=$consumerAccepted;candidate_without_receipt_accepted=$candidateAccepted
        candidate_reader_without_receipt_accepted=$candidateReaderAccepted
        receiptless_actual_accepted=$receiptlessAccepted;publication_resources_released=$resourcesReleased
    }
    foreach($key in $probeFields.Keys){$returnedSnapshot[$key]=$probeFields[$key]}
    $returnedSnapshot
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
    $cancelCase=$FixtureCase-in@('cancel','cancel_save_failure','cancel_publication_release_failure','cancel_post_emit_failure')
    $deadlineCase=$FixtureCase-in@('deadline','deadline_save_failure','deadline_publication_release_failure','deadline_post_emit_failure')
    $case=$(if($FixtureCase-in@('cwd_bound','cwd_other')){'cwd'}elseif($cancelCase-or$deadlineCase){'wait'}elseif($FixtureCase-in@('failure','failure_save_failure','failure_publication_release_failure','failure_post_emit_failure')){'failure'}else{'success'})
    $cancelBytes=$(if($cancelCase){131072}else{0})
    $budget=$(if($deadlineCase){1500}else{20000})
    $streamBudget=$(if($FixtureCase-in@('limit_save_failure','limit_publication_release_failure','limit_post_emit_failure')){65536}else{8388608})
    $selectedFixture=$FixtureProgram
    if($FixtureCase-in@('start_failure','start_finalize_save_failure','start_publish_replaced','start_post_move_pin_failure','start_publication_release_failure','start_post_emit_failure')){$selectedFixture=Join-Path $root 'civ6_autotest_rs-0000.exe'}
    $saveFault=''
    switch($FixtureCase){
        'save_partial_failure'{$saveFault='partial_write'}
        'save_flush_failure'{$saveFault='flush'}
        'save_dispose_failure'{$saveFault='dispose'}
        'publish_replaced'{$saveFault='replace_after_verify'}
        'start_publish_replaced'{$saveFault='replace_after_verify'}
        'publish_pin_protected'{$saveFault='pin_check'}
        'post_move_read_failure'{$saveFault='post_move_read'}
        'post_move_pin_failure'{$saveFault='post_move_pin'}
        'start_post_move_pin_failure'{$saveFault='post_move_pin'}
        'publication_release_failure'{$saveFault='release_failure'}
        'failure_publication_release_failure'{$saveFault='release_failure'}
        'start_publication_release_failure'{$saveFault='release_failure'}
        'cancel_publication_release_failure'{$saveFault='release_failure'}
        'deadline_publication_release_failure'{$saveFault='release_failure'}
        'limit_publication_release_failure'{$saveFault='release_failure'}
        'failure_post_emit_failure'{$saveFault='emit_failure'}
        'cancel_post_emit_failure'{$saveFault='emit_failure'}
        'deadline_post_emit_failure'{$saveFault='emit_failure'}
        'limit_post_emit_failure'{$saveFault='emit_failure'}
        'start_post_emit_failure'{$saveFault='emit_failure'}
        'post_emit_failure'{$saveFault='emit_failure'}
        'post_release_replacement'{$saveFault='replace_after_release'}
    }
    $output=Join-Path $root 'fixture.stdout.raw'
    $applicationName=$(if($FixtureCase-eq'resolved_success'){[IO.Path]::GetFileName($FixtureProgram)}else{''})
    $resolutionCase=$(switch($FixtureCase){'resolved_multiple'{'multiple'}'resolved_invalid'{'invalid'}'resolved_missing'{'missing'}'working_directory_missing'{'missing_cwd'}'cwd_other'{'other_cwd'}default{''}})
    $fixtureLabel=$(switch($FixtureCase){'public_label_git'{'git'}'public_label_unknown'{'unexpected-label'}'public_label_path'{'C:/private/public-phase-sentinel'}default{'fixture'}})
    $firstRecords=@(Invoke-Native-Capture -ApplicationName $applicationName -FixtureResolutionCase $resolutionCase -Program $selectedFixture -Arguments $fixtureArguments -RecordedArgv @('own-test-fixture') -OutputPath $output -Label $fixtureLabel -TimeoutMs $budget -StreamLimit $streamBudget -CancelAfterBytes $cancelBytes -FixtureChildCase $case -FixtureProfileCase $FixtureProfile -FixtureCleanupFault:($FixtureCase-eq'cleanup_finalize_save_failure') -FixtureFinalizeFault:($FixtureCase-in@('capture_finalize_save_failure','start_finalize_save_failure')) -FixtureResultSaveCase $saveFault)
    if($firstRecords.Count-ne1-or$firstRecords[0]-isnot[Collections.IDictionary]-or$firstRecords[0].schema-cne'civ6-private-native-capture/v1'){throw 'capture fixture primary return must be one capture record'}
    $first=$firstRecords[0]
    $events=@(Fixture-Capture-Summary $output $first)
    if($FixtureCase-in@('consume_unchanged','consume_modified','consume_replaced')){
        if($FixtureCase-eq'consume_modified'){
            $changed=[IO.File]::ReadAllBytes($output);$changed[0]=$changed[0]-bxor1
            [IO.File]::WriteAllBytes($output,$changed)
        }
        if($FixtureCase-eq'consume_replaced'){
            [IO.File]::Move($output,($output+'.original'))
            [IO.File]::WriteAllBytes($output,[Text.Encoding]::UTF8.GetBytes('fixture replacement'))
        }
        try{
            $text=Read-Captured-Text $output $first
            $events[0]['consumer_verified']=$true
            $events[0]['consumer_bytes']=[Text.Encoding]::UTF8.GetByteCount($text)
            $events[0]['consumer_sha256']=Digest-Text $text
        }catch{
            $events[0]['consumer_verified']=$false
            $events[0]['consumer_failure_stage']=$script:lastCaptureFailure.failure_stage
        }
    }
    if($FixtureCase-in@('cancel','deadline')){
        $nextOutput=Join-Path $root 'next.stdout.raw'
        $nextRecords=@(Invoke-Native-Capture -Program $FixtureProgram -Arguments $fixtureArguments -RecordedArgv @('own-test-fixture') -OutputPath $nextOutput -Label 'fixture-next' -TimeoutMs 20000 -StreamLimit 8388608 -FixtureChildCase 'success')
        if($nextRecords.Count-ne1-or$nextRecords[0]-isnot[Collections.IDictionary]-or$nextRecords[0].schema-cne'civ6-private-native-capture/v1'){throw 'capture fixture next return must be one capture record'}
        $next=$nextRecords[0]
        $events+=Fixture-Capture-Summary $nextOutput $next
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
        Require-Capture $result $log
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
    Require-Capture $generated ($prefix+'.generator.stdout.raw')
    Write-Output ('CIV_PRODUCT_BUNDLE source='+$after.source_sha+' tree='+$after.source_tree+' members=5')
}finally{$pin.Dispose()}
}catch{
    if($prefix){
        try{Save-New ($prefix+'.'+$Phase.ToLowerInvariant()+'.failure.json') ([ordered]@{error=$_.Exception.ToString();exit_code=$script:producerExit;capture=$script:lastCaptureFailure})}catch{}
    }
    $safeFailure=Diagnostic-Codes $_.Exception
    $safeFailure['schema']='civ6-product-phase-failure/v1'
    $safeFailure['phase']=$Phase
    $safeFailure['exit_code']=$script:producerExit
    [Console]::Out.WriteLine('CIV_PRODUCT_PHASE_FAILED '+($safeFailure|ConvertTo-Json -Compress))
    exit $script:producerExit
}
