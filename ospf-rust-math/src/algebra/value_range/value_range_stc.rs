use super::*;
use crate::algebra::*;

pub struct ValueRangeStc<T: Arithmetic, LI: IntervalType = Closed, UI: IntervalType = Closed> {
    lb: Option<BoundStc<T, LI>>,
    ub: Option<BoundStc<T, UI>>,
}
