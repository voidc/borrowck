#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_middle;

use rustc_driver::RunCompiler;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface::Compiler, Config, Queries};
use rustc_middle::ty;
use std::{env, process::Command};

struct DefaultCallbacks;
impl rustc_driver::Callbacks for DefaultCallbacks {}

fn main() {
    rustc_driver::init_rustc_env_logger();

    let mut args = env::args().collect::<Vec<_>>();

    let is_wrapper = args.get(1).map(|s| s.contains("rustc")).unwrap_or(false);

    if is_wrapper {
        args.remove(1);
    }

    let sysroot = sysroot_path();
    args.push(format!("--sysroot={}", sysroot));

    let mut callbacks = BorrowCk;

    RunCompiler::new(&args, &mut callbacks).run().unwrap();
}

fn sysroot_path() -> String {
    let channel = "nightly-2023-03-15";

    let output = Command::new("rustup")
        .arg("run")
        .arg(channel)
        .arg("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .unwrap();

    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

pub struct BorrowCk;

impl Callbacks for BorrowCk {
    fn config(&mut self, config: &mut Config) {
        config.override_queries = Some(|_sess, providers, _external_providers| {
            providers.mir_borrowck = |tcx, did| {
                if let Some(def) = ty::WithOptConstParam::try_lookup(did, tcx) {
                    tcx.mir_borrowck_const_arg(def)
                } else {
                    borrowck::mir_borrowck(tcx, ty::WithOptConstParam::unknown(did))
                }
            };
            providers.mir_borrowck_const_arg = |tcx, (did, param_did)| {
                borrowck::mir_borrowck(
                    tcx,
                    ty::WithOptConstParam { did, const_param_did: Some(param_did) },
                )
            };
            /*
            providers.mir_drops_elaborated_and_const_checked = |tcx, def_id| {
                let mir = (rustc_interface::DEFAULT_QUERY_PROVIDERS
                    .mir_drops_elaborated_and_const_checked)(tcx, def_id);
                borrowck::mir_borrowck(tcx, def_id);
                mir
            }
            */
        });
    }

    fn after_expansion<'tcx>(&mut self, c: &Compiler, queries: &'tcx Queries<'tcx>) -> Compilation {
        queries.global_ctxt().unwrap();
        let _ = queries.global_ctxt().unwrap().enter(|tcx| {
            let _ = tcx.analysis(());
        });

        c.session().abort_if_errors();

        Compilation::Stop
    }
}
