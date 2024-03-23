use std::fs::{self, read_dir, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

const DATALOG_DIR : &'static str = "datalog";
const DATALOG_COMPILED : &'static str = "datalog_compiled";

const PROGRAM_LIST_FILE : &'static str = "src/transformation/souffle/programs.rs";

fn create_program_list_file() -> BufWriter<File> {
    let list_file = File::create(PROGRAM_LIST_FILE).expect("Could not open program list file.");
    let mut writer = BufWriter::new(list_file);
    writeln!(writer, "\
#[cxx::bridge(namespace=\"souffle\")]
mod programs_ffi {{\
").expect("Could not write to program list file.");
    writer
}

fn close_program_list_file(mut writer : BufWriter<File>) {
    write!(writer, "}}").expect("Could not write to program list file.");
}

fn register_program(writer : &mut BufWriter<File>, filepath : PathBuf, name : &str) {
    writeln!(writer, "
    unsafe extern \"C++\" {{
        include!(\"{}\");
        type factory_Sf_{};
    }}\
",
    filepath.to_str().expect("Invalid path."), name).expect("Could not write to program list file.");
}

fn main() {
    let datalog_path = PathBuf::from(DATALOG_DIR)
        .canonicalize()
        .expect("No datalog directory.");

    let mut datalog_compiled_path = PathBuf::from(DATALOG_COMPILED);
    if !datalog_compiled_path.exists() {
        fs::create_dir_all(datalog_compiled_path.clone()).expect("Could not create directory {datalog_compiled_path}");
    }
    datalog_compiled_path = datalog_compiled_path.canonicalize().expect("Error computing path for compiled directory.");

    let mut program_list_writer = create_program_list_file();
    for dir in read_dir(datalog_path.clone()).expect("Could not open datalog dir.") {
        let path = dir.expect("Could not read file.").path();
        let progname = path
            .file_stem()
            .expect("Invalide file.")
            .to_str()
            .expect("Encoding error.")
            .to_owned();
        let outpath = datalog_compiled_path.join(progname.clone() + ".cpp");
        if !std::process::Command::new("souffle")
            .arg("-g")
            .arg(&outpath)
            .arg(path)
            .output()
            .expect("Could not find souffle.")
            .status
            .success()
        {
            panic!("Could not generate souffle program.");
        }
        register_program(&mut program_list_writer, outpath, &progname);
    }
    close_program_list_file(program_list_writer);

    cxx_build::bridge("src/transformation/souffle/mod.rs")
        .file("cpp_util/souffleUtil.hpp")
        .file("datalog_compiled/basic.cpp")
        .cpp(true)
        .std("c++17")
        .flag("-fkeep-inline-functions")
        .define("__EMBEDDED_SOUFFLE__", None)
        .include(".")
        .compile("transProofSouffle");

}
