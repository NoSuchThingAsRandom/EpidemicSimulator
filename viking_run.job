#!/bin/bash
#SBATCH --job-name=EpidemicSimulatorTesting     # Job name
#SBATCH --mail-type=NONE                        # Mail events (NONE, BEGIN, END, FAIL, ALL)
#SBATCH --mail-user=sr1474@york.ac.uk           # Where to send mail
#SBATCH --ntasks=1                              # Run a single task...
#SBATCH --cpus-per-task=32                      # ...with four cores
#SBATCH --mem=64gb                              # Job memory request
#SBATCH --time=12:00:00                         # Time limit hrs:min:sec
#SBATCH --output=epidemic_sim_v1.6_%j.log            # Standard output and error log
#SBATCH --account=cs-teach-2021                # Project account

echo My working directory is `pwd`
echo Running job on host:
echo -e '\t'`hostname` at `date`
echo $SLURM_CPUS_ON_NODE CPU cores available
echo

module load lang/Rust/1.54.0-GCCcore-11.2.0

export RUST_LOG="warn,visualisation,osm_data=trace,sim=trace,run=debug,load_census_data=trace,voronoice=off"
export RUST_BACKTRACE=1
#2013265923TYPE299
#1946157112TYPE299
echo "Log level: $RUST_LOG"
cargo run --release -- 1946157112TYPE299 --directory=data --grid-size=250000 --simulate  2>&1 | tee logs/viking_logs/v1.6/1946157112TYPE299.log
cargo run --release -- 2013265923TYPE299 --directory=data --grid-size=250000 --simulate  2>&1 | tee logs/viking_logs/v1.6/2013265923TYPE299.log

echo Job completed at `date`