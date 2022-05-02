export RUST_LOG="warn,visualisation,osm_data=trace,sim=trace,run=debug,load_census_data=trace,voronoice=off"
export RAYON_NUM_THREADS=40
export RUST_BACKTRACE=full
mkdir logs/pc_logs/v1.7.1
cargo run --release -- 1946157112TYPE299 --directory=data --grid-size=250000 --use-cache --simulate  2>&1 | tee logs/pc_logs/v1.7.1/1946157112TYPE299.log
#2013265923TYPE299
#1946157112TYPE299
