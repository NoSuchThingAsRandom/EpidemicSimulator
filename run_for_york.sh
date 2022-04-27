export RUST_LOG="warn,visualisation,osm_data=trace,sim=trace,run=debug,load_census_data=trace,voronoice=off"
export RAYON_NUM_THREADS=40
export RUST_BACKTRACE=full
cargo run --release -- 2013265923TYPE299 --directory=data --grid-size=250000 --simulate  2>&1 | tee logs/pc_logs/v1.6/2013265923TYPE299.log
#2013265923TYPE299
#1946157112TYPE299
