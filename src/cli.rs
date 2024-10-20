use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use proptest::proptest;

fn two_argument_parse(params: Vec<&str>) -> Option<(u64, u16, Option<SocketAddr>)> {
    if let Some(map_of_commands) = param_map_maker(params) {
        let maybe_port: Option<u16> = map_of_commands
            .get("port")
            .and_then(|port| port.parse::<u16>().ok());
        let maybe_period: Option<u64> = map_of_commands
            .get("period")
            .and_then(|period| period.parse::<u64>().ok());
        maybe_period.zip(maybe_port).map(|(period, port)| (period, port, None))
    } else {
        None
    }
}

fn three_argument_parse(params: Vec<&str>) -> Option<(u64, u16, Option<SocketAddr>)> {
    if let Some(map_of_commands) = param_map_maker(params) {
        let maybe_port: Option<u16> = map_of_commands
            .get("port")
            .and_then(|port| port.parse::<u16>().ok());
        let maybe_period: Option<u64> = map_of_commands
            .get("period")
            .and_then(|period| period.parse::<u64>().ok());
        let maybe_connect: Option<SocketAddr> = map_of_commands
            .get("connect")
            .and_then(|addr| addr.replace("\"", "").parse::<SocketAddr>().ok());
        maybe_period
            .zip(maybe_port)
            .zip(maybe_connect)
            .map(|((period, port), connect)| (period, port, Some(connect)))
    } else {
        None
    }
}

fn param_map_maker(params: Vec<&str>) -> Option<HashMap<&str, &str>> {
    let pairs: Vec<Vec<&str>> = params.iter().map(|str| str.split("=").collect()).collect();
    //"0" check is to avoid zero port or period
    if pairs.iter().all(|p| p.len() == 2 && p[1] != "0") {
        Some(pairs.iter().map(|p| (p[0], p[1])).into_iter().collect())
    } else {
        None
    }
}

fn figure_out_input(input: &str) -> Option<(u64, u16, Option<SocketAddr>)> {
    let mut command_vector = input.split(" --").collect::<Vec<&str>>();
    if command_vector.len() > 0 {
        match (command_vector.remove(0), command_vector.len()) {
            ("./peer", 3) => three_argument_parse(command_vector),
            ("./peer", 2) => two_argument_parse(command_vector),
            _ => None,
        }
    } else {
        None
    }
}

pub fn cli_loop() -> (u64, u16, Option<SocketAddr>) {
    loop {
        let mut input = String::new();

        println!("Enter the node startup command: ");

        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        if let Some(result) = figure_out_input(input.trim()) {
            break result;
        } else {
            println!("Malformed command, try again")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_former(elements: Vec<&str>, separator: &str) -> String {
        elements.join(separator)
    }


    #[test]
    fn one_parameter_false() {
        let value_to_compare = None;
        let result = figure_out_input(&*string_former(vec!["./peer", "period=5"], " --"));
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn four_parameter_false() {
        let value_to_compare = None;
        let result = figure_out_input(&*string_former(vec!["./peer", "period=5","port=5050", "connect=\"127.0.0.1:5000\"", "period=5"], " --"));
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn empty_string() {
        let value_to_compare = None;
        let result = figure_out_input(&*string_former(vec![], " --"));
        assert_eq!(result, value_to_compare);
    }

    #[test]
    fn wrong_command() {
        let value_to_compare = None;
        let result = figure_out_input(&*string_former(vec!["./pear", "period=5","port=5050", "connect=\"127.0.0.1:5000\"", "period=5"], " --"));
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn free_args_fun_correct() {
        let value_to_compare = Some((5u64, 5050u16, Some("127.0.0.1:5000".parse::<SocketAddr>().unwrap())));
        let result = three_argument_parse(vec!["period=5","port=5050", "connect=\"127.0.0.1:5000\""]);
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn free_args_fun_incorrect() {
        let value_to_compare = None;
        let result = three_argument_parse(vec!["period=5","port:5050", "connect=\"127.0.0.1:5000\""]);
        assert_eq!(result, value_to_compare);
    }

    #[test]
    fn two_args_fun_correct() {
        let value_to_compare = Some((5u64, 5050u16, None));
        let result = two_argument_parse(vec!["period=5","port=5050"]);
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn two_args_fun_incorrect() {
        let value_to_compare = None;
        let result = two_argument_parse(vec!["period=5","port:5050"]);
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn two_args_fun_missing() {
        let value_to_compare = None;
        let result = two_argument_parse(vec!["period=5"]);
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn two_args_fun_excessive() {
        let value_to_compare = None;
        let result = two_argument_parse(vec!["period=5","port:5050","period=5","port=5050"]);
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn correct_map_making_test() {
        let value_to_compare: Option<HashMap<&str, &str>> = Some(HashMap::from([("period", "5"), ("port", "5050")]));
        let result = param_map_maker(vec!["period=5","port=5050"]);
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn incorrect_map_making_test() {
        let value_to_compare: Option<HashMap<&str, &str>> = None;
        let result = param_map_maker(vec!["period:5","port:5050"]);
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn empty_map_making_test() {
        let value_to_compare: Option<HashMap<&str, &str>> = Some(HashMap::new());
        let result = param_map_maker(vec![]);
        assert_eq!(result, value_to_compare);
    }
    #[test]
    fn another_incorrect_map_making_test() {
        let value_to_compare: Option<HashMap<&str, &str>> = None;
        let result = param_map_maker(vec!["period=5=7","port=8=5050"]);
        assert_eq!(result, value_to_compare);
    }

    #[test]
    fn zero_value_map_making_test() {
        let value_to_compare: Option<HashMap<&str, &str>> = None;
        let result = param_map_maker(vec!["period=0","port=0"]);
        assert_eq!(result, value_to_compare);
    }

    proptest! {
        // Define the property-based test
        #[test]
        fn three_params_proptest(
            period in 0u64..=18446744073709551615u64,
            port_self in 0u16..=65535u16,
            ip_1 in 0u8..=255u8,
            ip_2 in 0u8..=255u8,
            ip_3 in 0u8..=255u8,
            ip_4 in 0u8..=255u8,
            port_server in 0u16..=65535u16
        ) {
            let text_address = format!("./peer --period={} --port={} --connect=\"{}.{}.{}.{}:{}\"", period, port_self, ip_1, ip_2, ip_3, ip_4, port_server);

            let result = figure_out_input(&*text_address);

            if period == 0 || port_self == 0 {
                assert!(result.is_none());
            } else {
                assert!(result.is_some());
            }
        }

        #[test]
        fn two_params_proptest(
            period in 0u64..=18446744073709551615u64,
            port_self in 0u16..=65535u16,
        ) {
            let text_address = format!("./peer --period={} --port={}", period, port_self);

            let result = figure_out_input(&*text_address);

            if period == 0 || port_self == 0 {
                assert!(result.is_none());
            } else {
                assert!(result.is_some());
            }
        }
    }
}

