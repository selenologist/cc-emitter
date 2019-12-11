#![feature(result_map_or_else)]

extern crate structopt;

use midir::{MidiOutput, MidiOutputConnection};
use regex::Regex;
use structopt::StructOpt;

// program arguments
#[derive(StructOpt)]
struct Opts {
    /// Connect only to ports whose name contains a given string (defaults to connecting to all ports)
    #[structopt(short = "p", long = "port")]
    port_filter: Option<String>,

    /// Send messages on only a specific channel (defaults to sending to all 16 channels)
    #[structopt(short = "c", long = "channel")]
    channel: Option<u8>,

    /// Enable extra verbosity. Defaults to disabled.
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,

    /// MIDI CC data to send, in the following format:
    ///
    /// (<CC>:<Value>[^:0-9]*)+
    ///
    /// That is, the CC number and value should be joined by :, and separated from each other by
    /// any other character.
    ///
    /// Both the CC number and value should be decimals within the range [0-127].
    ///
    /// Example: "70:104 74:124,122:0" will send 104 to CC#104, 124 to #74, 0 to #122, etc.
    data: String
}

// Display name for output port
const OUTPUT_PORT_NAME: &'static str = "@selenologist CC emitter";
// Name to be displayed on connections
const OUTPUT_CONNECTION_NAME: &'static str = "@selenologist CC emitter connection";

// MIDI protocol constants
const CONTROL_CHANGE_PREFIX: u8 = 0xB0;

fn main() {
    // parse program arguments
    let opts = Opts::from_args();
    
    // compile regex for parsing CC input
    let cc_regex = Regex::new(r"([0-9]+):([0-9]+)").expect("Failed to create CC regex");

    // convert CC input string into (CC, Value) u8 pairs
    let data: Vec<(u8, u8)> = cc_regex
        .captures_iter(opts.data.as_str())
        .map(|cap| {
            let str_to_u8 = |s: &str| {
                let i = s
                    .parse::<isize>()
                    .unwrap_or_else(|_| panic!("Data value '{}' is could not be parsed.", s));

                // if the value is out of the unsigned 8-bit range
                if i < 0 || i > 255 {
                    // note, this program will happily attempt to send CCs greater than 127
                    // what happens to the output when you do this is undefined.
                    panic!("Data value '{}' is out of range.", i);
                }
                else {
                    i as u8
                }
            };

            let cc    = cap.get(1).unwrap().as_str();
            let value = cap.get(2).unwrap().as_str();

            (str_to_u8(cc), str_to_u8(value))
        })
        .collect();

    // emit data on the specified channels for a given connection
    let do_conn = |mut conn: MidiOutputConnection| {
        let mut do_channel = |channel: u8| {
            for (cc, value) in data.iter() {
                if opts.verbose {
                    println!("Sending CC#{} value {} on ch#{}", cc, value, channel+1);
                }
                
                conn.send(&[CONTROL_CHANGE_PREFIX | channel, *cc, *value])
                    .unwrap_or_else(|e| eprintln!("Failed to send CC#{} value {} on ch#{}: {:?}",
                                                  cc, value, channel+1, e));
            }
        };

        // if a specific channel was supplied as an argument, only do that channel
        if let Some(specified_channel) = opts.channel {
            // convert from human 1-based channel index, to 0-based indexing.
            let channel = match specified_channel {
                // treat input of 0 as being synonymous with channel 1
                0      => 0,
                // if between 1 and 16, subtract 1 to convert to zero-indexed
                1..=16 => specified_channel - 1,
                // otherwise an invalid channel was specified, panic.
                _      => panic!("Channel {} exceeds maximum of 16", specified_channel)
            };

            do_channel(channel as u8)
        }
        // otherwise, send to all channels
        else {
            (0u8..16).for_each(do_channel)
        }
    };

    // create a MIDI output
    let make_output = ||
        MidiOutput::new(OUTPUT_PORT_NAME)
            .expect("Failed to open MIDI output");

    let mut output = make_output();

    // connect to each available port, filtering by name if specified
    //
    // note: the interface of midir, just like most underlying platform APIs, is inherently prone
    // to race conditions regarding the port number.
    // Hopefully the program completes fast enough that this doesn't cause annoying effects in
    // practice. The program will not crash at least.
    for port in 0..output.port_count() {
        // get the port's name
        let name = match output.port_name(port) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Failed to get port #{} name: {:?}. Skipping this port.", port, e);
                continue;
            }
        };

        // if a name filter is set, check if the port name matches it
        if let Some(ref filter) = opts.port_filter {
            if !name.contains(filter) {
                if opts.verbose {
                    println!("Skipping port #{} \"{}\" because it doesn't contain \"{}\"",
                         port, name, filter);
                }
                continue;
            }
        }

        if opts.verbose {
            println!("Connecting to port #{} \"{}\"", port, name);
        }

        // hack: MidiOutput.connect consumes the MidiOutput, but a MidiOutput is necessary to
        // query the ports! So, replace the old MidiOutput with a new one so we can take ownership
        // of the old one.
        // This creates a wasted extra MidiOutput at the end of the loop but meh.
        // Avoids having to write an even messier way of maintaining already-done port information.
        // Doing this at this point also avoids recreating output if we skipped the current port.
        let current_output = std::mem::replace(&mut output, make_output()); 

        current_output
            .connect(port, OUTPUT_CONNECTION_NAME)
            .map_or_else(|e| eprintln!("Failed to connect to port#{} \"{}\": {:?}", port, name, e),
                         do_conn);
    }
}
