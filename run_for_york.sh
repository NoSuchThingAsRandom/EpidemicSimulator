RUST_LOG="warn,visualisation,osm_data=trace,sim=trace,run=debug,load_census_data=trace,voronoice=off"
RUST_BACKTRACE=1
cargo run --release -- 2013265923TYPE299 --directory=data --grid-size=250000 --simulate --use-cache &>logs/york_and_humber/log_multithreaded_exposure_gen.log