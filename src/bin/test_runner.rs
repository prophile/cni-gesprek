use cni_gesprek::testing::*;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    let binary_path = if args.len() > 1 {
        args[1].clone()
    } else {
        // Try to find the binary in target directory
        let mut path = env::current_exe().unwrap();
        path.pop();
        if path.ends_with("deps") {
            path.pop();
        }
        path.push("cni-gesprek");
        path.to_string_lossy().to_string()
    };

    println!("Running white-box tests for: {}", binary_path);
    println!("========================================");

    let runner = TestRunner::new(&binary_path);
    
    let test_cases: Vec<Box<dyn TestCase>> = vec![
        Box::new(BasicAddTest::new()),
        Box::new(AddWithDnsTest::new()),
        Box::new(AutoDetectTest::new()),
        Box::new(SubnetDetectTest::new()),
        Box::new(VersionTest::new()),
        Box::new(DeleteTest::new()),
        Box::new(CheckTest::new()),
        Box::new(StatusTest::new()),
    ];

    let mut passed = 0;
    let mut failed = 0;

    for test_case in test_cases {
        let test_name = test_case.name().to_string();
        print!("Running {:<30}", test_name);
        
        match runner.run_test(test_case) {
            Ok(result) => {
                if result.is_success() {
                    println!(" [PASS]");
                    passed += 1;
                } else {
                    println!(" [FAIL]");
                    println!("{}", result);
                    failed += 1;
                }
            }
            Err(e) => {
                println!(" [ERROR]");
                println!("Error running test: {}", e);
                failed += 1;
            }
        }
    }

    println!("\n========================================");
    println!("Test Results: {} passed, {} failed", passed, failed);
    
    if failed > 0 {
        std::process::exit(1);
    }
}
