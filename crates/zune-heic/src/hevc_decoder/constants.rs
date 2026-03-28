#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PartMode {
    Part2Nx2N = 0,
    Part2NxN  = 1,
    PartNx2N  = 2,
    PartNxN   = 3,
    Part2NxnU = 4,
    Part2NxnD = 5,
    PartnLx2N = 6,
    PartnRx2N = 7,
}