# V1.1 - Interventions

#### (Git Commit: ef83899d)

### Notes:

First version with Interventions:

* Lockdown
* Mask Wearing
* Vaccinations

Massive slow down occurs with calculating exposures

### Stats:

Was the Yorkshire and Humber Area with:

* 17246 Output Areas
* 5249772 Citizens

### Loading Time:

INFO sim::models    > Finished loading map data in 1.452593974s \
INFO sim::simulator > Loaded map data in 1.29846196s\
INFO sim::simulator > Built residential population in 11.84226115s\
INFO sim::simulator > Generated workplaces in 16.28118846s\
INFO sim::simulator > Initialization completed in 16.315647 seconds

### Configuration:

Citizen Distribution Config:

```rust
pub const STARTING_INFECTED_COUNT: u32 = 10;
/// The amount of floor space in m^2 per Workplace building
pub const WORKPLACE_BUILDING_SIZE: u16 = 1000;
pub const HOUSEHOLD_SIZE: u16 = 4;

/// How often to print debug statements
pub const DEBUG_ITERATION_PRINT: usize = 10;
```

Disease Config:

```rust
    pub fn covid() -> DiseaseModel {
        DiseaseModel {
            reproduction_rate: 2.5,
            exposure_chance: 0.4,
            death_rate: 0.2,
            exposed_time: 4 * 24,
            infected_time: 14 * 24,
            max_time_step: 1000,
        }
    }
```

# V1.0.1

#### (Git Commit: b31ede6d)

### Notes:

This was the first working result with the fixed, Citizen movement.

There was a bug in which after the first day, the Citizen would remain fixed at the same Building and the schedule would
not update.

### Stats:

Was the York Area with:

* 637 Output Areas
* 197080 Citizens
* There are 197080 nodes and 5659318 edges

### Loading Time:

INFO sim::models    > Finished loading map data in 1.452593974s \
INFO sim::simulator > Loaded map data in 1.623514537s\
INFO sim::simulator > Built residential population in 1.990680346s\
INFO sim::simulator > Generated workplaces in 2.229961886s\
INFO sim::simulator > Initialization completed in 2.2312038 seconds

### Configuration:

Citizen Distribution Config:

```rust
pub const STARTING_INFECTED_COUNT: u32 = 10;
/// The amount of floor space in m^2 per Workplace building
pub const WORKPLACE_BUILDING_SIZE: u16 = 1000;
pub const HOUSEHOLD_SIZE: u16 = 4;

/// How often to print debug statements
pub const DEBUG_ITERATION_PRINT: usize = 10;
```

Disease Config:

```rust
    pub fn covid() -> DiseaseModel {
        DiseaseModel {
            reproduction_rate: 2.5,
            exposure_chance: 0.4,
            death_rate: 0.2,
            exposed_time: 4 * 24,
            infected_time: 14 * 24,
            max_time_step: 1000,
        }
    }
```