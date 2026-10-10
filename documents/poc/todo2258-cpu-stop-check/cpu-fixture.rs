#[cfg(all(test, windows))]
mod tests {
    use cc_live::{args, chain, path, process::CREATE_NO_WINDOW, Call, Output, Paths, Runner};
    use serde_json::json;
    use std::{fs::{self, File, OpenOptions}, os::windows::fs::OpenOptionsExt, path::PathBuf};

    const ORIGINAL: &[u8] = b"DeviceID 5056\r\nAllowSWRenderer 0\r\nOther 9\r\n";
    const CPU: &[u8] = b"DeviceID 140\r\nAllowSWRenderer 1\r\nOther 9\r\n";

    struct Fixture { root: PathBuf, p: Paths, settings: PathBuf }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("todo2258-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            let p = Paths { here: root.join("here"), wt: root.join("wt"), jobs: root.join("jobs"), temp: root.join("temp"), profile: root.join("profile"), python: "unused".into() };
            for dir in [&p.here,&p.wt,&p.jobs,&p.temp,&p.profile] { fs::create_dir_all(dir).unwrap(); }
            let settings=p.profile.join("AppData/Local/Firaxis Games/Sid Meier's Civilization VI/AppOptions.txt");
            fs::create_dir_all(settings.parent().unwrap()).unwrap();
            fs::write(&settings, ORIGINAL).unwrap();
            let manifest=p.live().join(cc_live::driver::MANIFEST);
            fs::create_dir_all(manifest.parent().unwrap()).unwrap();
            fs::write(manifest,"[package]\nname='civ6-autotest-rs'\nversion='0.1.0'\n").unwrap();
            let bin=p.live().join("tools/target/release");
            fs::create_dir_all(&bin).unwrap();
            for name in ["civ6-autotest.exe","civ6-tuner.exe"] { fs::write(bin.join(name),b"fixture; never executed").unwrap(); }
            fs::write(root.join("scenario.json"), br#"{"steps":[{"action":"restart_game","timeout":1}]}"#).unwrap();
            Self { root,p,settings }
        }
        fn backup(&self)->PathBuf { self.p.here.join("software-renderer-original.bin") }
        fn lock(&self)->PathBuf { self.p.here.join("civ6.lock") }
        fn interrupted(&self) { fs::write(self.backup(),ORIGINAL).unwrap(); fs::write(&self.settings,CPU).unwrap(); }
        fn assert_restored(&self) { assert_eq!(fs::read(&self.settings).unwrap(),ORIGINAL); assert!(!self.backup().exists()); assert!(!self.lock().exists()); }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _=fs::remove_dir_all(&self.root); } }

    #[derive(Clone,Copy,PartialEq)]
    enum Mode { Normal, DriverError, DriverPanic, Interrupted, PrepareFailsFirst, RestoreBlocked }
    struct Model<'a> { f: &'a Fixture, mode: Mode, cpu: bool, alive: bool, prepares:usize, drivers:usize, closes:usize, time:f64, blocker:Option<File> }
    impl<'a> Model<'a> {
        fn new(f:&'a Fixture, mode:Mode,cpu:bool)->Self { Self{f,mode,cpu,alive:false,prepares:0,drivers:0,closes:0,time:1000.0,blocker:None} }
    }
    impl Runner for Model<'_> {
        fn run(&mut self,call:Call)->Result<Output,String> {
            assert_eq!(call.creationflags,CREATE_NO_WINDOW);
            let executable=call.args.first().unwrap();
            let mut result=Output::default();
            if executable=="tasklist" {
                if self.alive && call.args.iter().any(|x| x=="IMAGENAME eq CivilizationVI.exe") { result.stdout="\"CivilizationVI.exe\",\"900001\"".into(); }
            } else if executable.ends_with("civ6-tuner.exe") {
                match call.args.get(1).map(String::as_str) {
                    Some("states") => result.stdout="Civilization VI\n0: FrontEnd\n".into(),
                    Some("lua") => {
                        if self.cpu && self.drivers>0 { assert!(fs::remove_file(self.f.lock()).is_err(),"profile must protect lease until close"); }
                        self.alive=false; self.closes+=1;
                    },
                    _=>return Err(format!("unexpected tuner fixture call {:?}",call.args)),
                }
            } else if executable=="powershell" {
                assert!(!self.alive,"reporter cleanup must follow game exit");
            } else if executable.ends_with("ship.exe") {
                self.prepares+=1;
                assert!(!self.alive);
                assert_eq!(fs::read(&self.f.settings).unwrap(),if self.cpu {CPU} else {ORIGINAL});
                if self.mode==Mode::PrepareFailsFirst && self.prepares==1 { result.code=7; }
                else { result.stdout=format!("{}\n{}\n",path(self.f.root.join("scenario.json")),path(self.f.root.join(format!("artifacts{}",self.prepares)))); }
            } else if executable.ends_with("civ6-autotest.exe") {
                self.drivers+=1; self.alive=true;
                assert_eq!(fs::read(&self.f.settings).unwrap(),if self.cpu {CPU} else {ORIGINAL});
                match self.mode {
                    Mode::DriverError=>return Err("fixture driver error".into()),
                    Mode::DriverPanic=>panic!("fixture driver panic"),
                    Mode::Interrupted=>return Err("interrupted".into()),
                    Mode::RestoreBlocked=>self.blocker=Some(OpenOptions::new().read(true).share_mode(1).open(&self.f.settings).unwrap()),
                    _=>{},
                }
            } else { return Err(format!("unexpected fixture call {:?}",call.args)); }
            Ok(result)
        }
        fn now(&mut self)->f64 { self.time }
        fn sleep(&mut self,seconds:f64)->Result<(),String> { self.time+=seconds; if self.time>1200.0 {return Err("fixture exceeded bound".into())} Ok(()) }
        fn print(&mut self,_:&str)->Result<(),String> {Ok(())}
        fn error(&mut self,_:&str)->Result<(),String> {Ok(())}
    }

    #[test]
    fn cpu_two_normal_scenarios_restore_and_release() {
        let f=Fixture::new(); let mut m=Model::new(&f,Mode::Normal,true);
        assert_eq!(chain::cli(&mut m,&f.p,&args(&["--software-rendering","one:s.json","two:s.json"])).unwrap(),0);
        assert_eq!((m.prepares,m.drivers,m.closes),(2,2,2)); f.assert_restored();
        println!("CPU_FIXTURE={}",json!({"case":"normal_two","prepares":m.prepares,"drivers":m.drivers,"closed":m.closes,"original_restored":true,"lease_released":true,"game_live":false}));
    }
    #[test]
    fn cpu_driver_error_restores_before_release() { check_error(Mode::DriverError,"fixture driver error"); }
    #[test]
    fn cpu_panic_restores_before_release() { check_error(Mode::DriverPanic,"panicked"); }
    #[test]
    fn cpu_interrupt_result_restores_before_release() { check_error(Mode::Interrupted,"interrupted"); }
    fn check_error(mode:Mode,message:&str) {
        let f=Fixture::new(); let mut m=Model::new(&f,mode,true);
        let result=chain::cli(&mut m,&f.p,&args(&["--software-rendering","one:s.json"]));
        assert!(result.as_ref().is_err_and(|e|e.contains(message)),"{result:?}");
        assert_eq!((m.prepares,m.drivers,m.closes),(1,1,1)); f.assert_restored();
        println!("CPU_FIXTURE={}",json!({"case":message,"original_restored":true,"lease_released":true,"game_live":false,"interrupt_is_injected_result":mode==Mode::Interrupted}));
    }
    #[test]
    fn ordinary_run_recovers_interrupted_profile() {
        let f=Fixture::new(); f.interrupted(); let mut m=Model::new(&f,Mode::Normal,false);
        assert_eq!(chain::cli(&mut m,&f.p,&args(&["one:s.json"])).unwrap(),0); assert_eq!(m.drivers,1); f.assert_restored();
    }
    #[test]
    fn profile_restore_refusal_keeps_backup_and_lease_record() {
        let f=Fixture::new(); let mut m=Model::new(&f,Mode::RestoreBlocked,true);
        let result=chain::cli(&mut m,&f.p,&args(&["--software-rendering","one:s.json"]));
        assert!(result.as_ref().is_err_and(|e|e.contains("restoration pending")),"{result:?}");
        assert_eq!(fs::read(f.backup()).unwrap(),ORIGINAL); assert_eq!(fs::read(&f.settings).unwrap(),CPU); assert!(f.lock().exists());
        println!("CPU_FIXTURE={}",json!({"case":"active_restore_refusal","backup_retained":true,"lease_record_retained":true,"game_live":false}));
    }
    #[test]
    fn recovery_replace_refusal_must_preserve_lease() {
        let f=Fixture::new(); f.interrupted();
        let _blocker=OpenOptions::new().read(true).share_mode(1).open(&f.settings).unwrap();
        let mut m=Model::new(&f,Mode::Normal,false);
        let result=chain::cli(&mut m,&f.p,&args(&["one:s.json"]));
        assert!(result.is_err()); assert_eq!(m.drivers,0);
        assert_eq!(fs::read(f.backup()).unwrap(),ORIGINAL); assert_eq!(fs::read(&f.settings).unwrap(),CPU);
        println!("CPU_FIXTURE={}",json!({"case":"ordinary_recovery_replace_refusal","error":result.err(),"backup_retained":f.backup().exists(),"lease_record_retained":f.lock().exists(),"settings_remain_cpu":true,"driver_calls":m.drivers,"game_live":false}));
        assert!(f.lock().exists(),"failed recovery released standard lease while CPU settings remained");
    }
    #[test]
    fn prepare_failure_must_continue_next_scenario_without_stop_flag() {
        let f=Fixture::new(); let mut m=Model::new(&f,Mode::PrepareFailsFirst,true);
        let result=chain::cli(&mut m,&f.p,&args(&["--software-rendering","one:s.json","two:s.json"]));
        println!("CPU_FIXTURE={}",json!({"case":"prepare_failure_continue","result":format!("{result:?}"),"prepare_calls":m.prepares,"driver_calls":m.drivers,"game_live":false}));
        f.assert_restored();
        assert_eq!(m.prepares,2,"without stop-on-failure second preparation must be attempted");
        assert_eq!(m.drivers,1); assert_eq!(result.unwrap(),7);
    }
}
