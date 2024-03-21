use std::ops::{Add, Div, Mul, Sub};

use crate::algebra::concept::{ Arithmetic, RealNumber, Infinity, NegativeInfinity };

use super::*;

pub enum ValueWrapper<T: Arithmetic> {
    Value(T),
    Inf,
    NegInf,
}

impl<T: Arithmetic> From<Infinity> for ValueWrapper<T> {
    fn from(_: Infinity) -> Self {
        Self::Inf
    }
}

impl<T: Arithmetic> From<NegativeInfinity> for ValueWrapper<T> {
    fn from(_: NegativeInfinity) -> Self {
        Self::NegInf
    }
}

impl<T: Arithmetic> From<T> for ValueWrapper<T> {
    default fn from(value: T) -> Self {
        Self::Value(value)
    }
}

impl<T: RealNumber> From<T> for ValueWrapper<T> {
    fn from(value: T) -> Self {
        if value.is_inf() {
            Self::Inf
        } else if value.is_neg_inf() {
            Self::NegInf
        } else if value.is_nan() {
            panic!("Illegal argument NaN for value range!!!")
        } else {
            Self::Value(value)
        }
    }
}

impl<T: Arithmetic + for<'a> From<&'a U>, U: Arithmetic> From<&ValueWrapper<U>> for ValueWrapper<T> {
    fn from(value: &ValueWrapper<U>) -> Self {
        match value {
            ValueWrapper::Value(value) => ValueWrapper::from(T::from(value)),
            ValueWrapper::Inf => ValueWrapper::Inf,
            ValueWrapper::NegInf => ValueWrapper::NegInf,
        }
    }
}

impl<T: Arithmetic> ValueWrapper<T> {
    fn to<U: Arithmetic>(self) -> ValueWrapper<U> where U: From<T> {
        match self {
            Self::Value(value) => ValueWrapper::from(U::from(value)),
            Self::Inf => ValueWrapper::Inf,
            Self::NegInf => ValueWrapper::NegInf,
        }
    }
}

impl<T: Arithmetic + Clone> Clone for ValueWrapper<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Value(value) => ValueWrapper::from(value.clone()),
            Self::Inf => ValueWrapper::Inf,
            Self::NegInf => ValueWrapper::NegInf,
        }
    }
}

impl<T: Arithmetic + Copy> Copy for ValueWrapper<T> {}

impl<T: Arithmetic + std::fmt::Display> std::fmt::Display for ValueWrapper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Value(value) => write!(f, "{}", value),
            Self::Inf => write!(f, "inf"),
            Self::NegInf => write!(f, "-inf"),
        }
    }
}

impl<T: Arithmetic + std::fmt::Display> std::fmt::Debug for ValueWrapper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Value(value) => write!(f, "{}", value),
            Self::Inf => write!(f, "inf"),
            Self::NegInf => write!(f, "-inf"),
        }
    }
}

impl<T: Arithmetic> PartialEq for ValueWrapper<T> {
    default fn eq(&self, rhs: &Self) -> bool {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => lhs_value == rhs_value,
                _ => false,
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Inf => true,
                _ => false,
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::NegInf => true,
                _ => false,
            },
        }
    }
}

impl<T: Arithmetic> Eq for ValueWrapper<T> {}

impl<T: Arithmetic> PartialOrd for ValueWrapper<T> {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => lhs_value.partial_cmp(rhs_value),
                ValueWrapper::Inf => Some(std::cmp::Ordering::Less),
                ValueWrapper::NegInf => Some(std::cmp::Ordering::Greater),
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Some(std::cmp::Ordering::Greater),
                ValueWrapper::Inf => Some(std::cmp::Ordering::Equal),
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Some(std::cmp::Ordering::Less),
                ValueWrapper::NegInf => Some(std::cmp::Ordering::Equal),
            },
        }
    }
}

impl<T: Arithmetic> Ord for ValueWrapper<T> {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        self.partial_cmp(rhs).unwrap()
    }
}

impl<T: Arithmetic, U: Arithmetic> PartialEq<U> for ValueWrapper<T> where T: PartialEq<U> {
    default fn eq(&self, rhs: &U) -> bool {
        match self {
            ValueWrapper::Value(lhs_value) => lhs_value == rhs,
            _ => false,
        }
    }
}

impl<T: Arithmetic, U: RealNumber> PartialEq<U> for ValueWrapper<T> where T: PartialEq<U> {
    fn eq(&self, rhs: &U) -> bool {
        if rhs.is_nan() {
            false
        } else if rhs.is_inf() {
            self == &ValueWrapper::<T>::Inf
        } else if rhs.is_neg_inf() {
            self == &ValueWrapper::<T>::NegInf
        } else {
            if let ValueWrapper::Value(lhs_value) = self {
                lhs_value == rhs
            } else {
                false
            }
        }
    }
}

impl<T: Arithmetic, U: Arithmetic> PartialOrd<U> for ValueWrapper<T> where T: PartialOrd<U> {
    default fn partial_cmp(&self, rhs: &U) -> Option<std::cmp::Ordering> {
        match self {
            ValueWrapper::Value(lhs_value) => lhs_value.partial_cmp(rhs),
            ValueWrapper::Inf => Some(std::cmp::Ordering::Greater),
            ValueWrapper::NegInf => Some(std::cmp::Ordering::Less),
        }
    }
}

impl<T: Arithmetic, U: RealNumber> PartialOrd<U> for ValueWrapper<T> where T: PartialOrd<U> {
    fn partial_cmp(&self, rhs: &U) -> Option<std::cmp::Ordering> {
        if rhs.is_nan() {
            None
        } else {
            if rhs.is_inf() {
                match self {
                    ValueWrapper::Value(_) | ValueWrapper::NegInf => Some(std::cmp::Ordering::Less),
                    ValueWrapper::Inf => Some(std::cmp::Ordering::Equal),
                }
            } else if rhs.is_neg_inf() {
                match self {
                    ValueWrapper::Value(_) | ValueWrapper::Inf => Some(std::cmp::Ordering::Greater),
                    ValueWrapper::NegInf => Some(std::cmp::Ordering::Equal),
                }
            } else {
                match self {
                    ValueWrapper::Value(lhs_value) => lhs_value.partial_cmp(rhs),
                    ValueWrapper::Inf => Some(std::cmp::Ordering::Greater),
                    ValueWrapper::NegInf => Some(std::cmp::Ordering::Less),
                }
            }
        }
    }
}

impl<T: Arithmetic, U: Arithmetic, V: Arithmetic> Add<U> for ValueWrapper<T>
    where T: Add<U, Output=V>,
{
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    default fn add(self, rhs: U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value + rhs)),
            ValueWrapper::Inf => Ok(ValueWrapper::Inf),
            ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
        }
    }
}

impl<T: Arithmetic, U: RealNumber, V: Arithmetic> Add<U> for ValueWrapper<T>
    where T: Add<U, Output=V>,
{
    fn add(self, rhs: U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        if rhs.is_nan() {
            return Err(IllegalArgumentError {
                msg: String::from("Illegal argument NaN for value range!!!"),
            });
        }

        if rhs.is_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid plus between inf and -inf!!!"),
                }),
            }
        } else if rhs.is_neg_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid plus between inf and -inf!!!"),
                }),
            }
        } else {
            match self {
                ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value + rhs)),
                ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
            }
        }
    }
}

impl<T: Arithmetic, V: Arithmetic> Add for ValueWrapper<T> where T: Add<Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    fn add(self, rhs: ValueWrapper<T>) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => Ok(ValueWrapper::from(lhs_value + rhs_value)),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
                ValueWrapper::Inf => Ok(ValueWrapper::Inf),
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid plus between inf and -inf!!!"),
                }),
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid plus between inf and -inf!!!"),
                }),
            },
        }
    }
}

impl<T: Arithmetic, U: Arithmetic, V: Arithmetic> Add<&U> for &ValueWrapper<T>
    where for<'a> &'a T: Add<&'a U, Output=V>,
{
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    default fn add(self, rhs: &U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value + rhs)),
            ValueWrapper::Inf => Ok(ValueWrapper::Inf),
            ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
        }
    }
}

impl<T: Arithmetic, U: RealNumber, V: Arithmetic> Add<&U> for &ValueWrapper<T>
    where for<'a> &'a T: Add<&'a U, Output=V>,
{
    fn add(self, rhs: &U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        if rhs.is_nan() {
            return Err(IllegalArgumentError {
                msg: String::from("Illegal argument NaN for value range!!!"),
            });
        }

        if rhs.is_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid plus between inf and -inf!!!"),
                }),
            }
        } else if rhs.is_neg_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid plus between inf and -inf!!!"),
                }),
            }
        } else {
            match self {
                ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value + rhs)),
                ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
            }
        }
    }
}

impl<T: Arithmetic, V: Arithmetic> Add for &ValueWrapper<T> where for<'a> &'a T: Add<&'a T, Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    fn add(self, rhs: &ValueWrapper<T>) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => Ok(ValueWrapper::from(lhs_value + rhs_value)),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
                ValueWrapper::Inf => Ok(ValueWrapper::Inf),
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid plus between inf and -inf!!!"),
                }),
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid plus between inf and -inf!!!"),
                }),
            },
        }
    }
}

impl<T: Arithmetic, U: Arithmetic, V: Arithmetic> Sub<U> for ValueWrapper<T> where T: Sub<U, Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    default fn sub(self, rhs: U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value - rhs)),
            ValueWrapper::Inf => Ok(ValueWrapper::Inf),
            ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
        }
    }
}

impl<T: Arithmetic, U: RealNumber, V: Arithmetic> Sub<U> for ValueWrapper<T> where T: Sub<U, Output=V> {
    fn sub(self, rhs: U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        if rhs.is_nan() {
            return Err(IllegalArgumentError {
                msg: String::from("Illegal argument NaN for value range!!!"),
            });
        }

        if rhs.is_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid sub between inf and inf!!!"),
                }),
            }
        } else if rhs.is_neg_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid sub between -inf and -inf!!!"),
                }),
            }
        } else {
            match self {
                ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value - rhs)),
                ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
            }
        }
    }
}

impl<T: Arithmetic, V: Arithmetic> Sub for ValueWrapper<T> where T: Sub<Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    fn sub(self, rhs: ValueWrapper<T>) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => Ok(ValueWrapper::from(lhs_value - rhs_value)),
                ValueWrapper::Inf => Ok(ValueWrapper::NegInf),
                ValueWrapper::NegInf => Ok(ValueWrapper::Inf),
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Ok(ValueWrapper::Inf),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid sub between inf and inf!!!"),
                }),
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::NegInf),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid sub between -inf and -inf!!!"),
                }),
            },
        }
    }
}

impl<T: Arithmetic, U: Arithmetic, V: Arithmetic> Sub<&U> for &ValueWrapper<T> where for<'a> &'a T: Sub<&'a U, Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    default fn sub(self, rhs: &U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value - rhs)),
            ValueWrapper::Inf => Ok(ValueWrapper::Inf),
            ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
        }
    }
}

impl<T: Arithmetic, U: RealNumber, V: Arithmetic> Sub<&U> for &ValueWrapper<T> where for<'a> &'a T: Sub<&'a U, Output=V> {
    fn sub(self, rhs: &U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        if rhs.is_nan() {
            return Err(IllegalArgumentError {
                msg: String::from("Illegal argument NaN for value range!!!"),
            });
        }

        if rhs.is_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid sub between inf and inf!!!"),
                }),
            }
        } else if rhs.is_neg_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid sub between -inf and -inf!!!"),
                }),
            }
        } else {
            match self {
                ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value - rhs)),
                ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
            }
        }
    }
}

impl<T: Arithmetic, V: Arithmetic> Sub for &ValueWrapper<T> where for<'a> &'a T: Sub<&'a T, Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    fn sub(self, rhs: &ValueWrapper<T>) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => Ok(ValueWrapper::from(lhs_value - rhs_value)),
                ValueWrapper::Inf => Ok(ValueWrapper::NegInf),
                ValueWrapper::NegInf => Ok(ValueWrapper::Inf),
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::NegInf => Ok(ValueWrapper::Inf),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid sub between inf and inf!!!"),
                }),
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::NegInf),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid sub between -inf and -inf!!!"),
                }),
            },
        }
    }
}

impl<T: Arithmetic, U: Arithmetic, V: Arithmetic> Mul<U> for ValueWrapper<T> where T: Mul<U, Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    default fn mul(self, rhs: U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value * rhs)),
            ValueWrapper::Inf => {
                if &rhs >= U::ZERO {
                    Ok(ValueWrapper::Inf)
                } else {
                    Ok(ValueWrapper::NegInf)
                }
            }
            ValueWrapper::NegInf => {
                if &rhs >= U::ZERO {
                    Ok(ValueWrapper::NegInf)
                } else {
                    Ok(ValueWrapper::Inf)
                }
            }
        }
    }
}

impl<T: Arithmetic, U: RealNumber, V: Arithmetic> Mul<U> for ValueWrapper<T> where T: Mul<U, Output=V> {
    fn mul(self, rhs: U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        if rhs.is_nan() {
            return Err(IllegalArgumentError {
                msg: String::from("Illegal argument NaN for value range!!!"),
            });
        }

        if rhs.is_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
            }
        } else if rhs.is_neg_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::NegInf),
                ValueWrapper::NegInf => Ok(ValueWrapper::Inf),
            }
        } else {
            match self {
                ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value * rhs)),
                ValueWrapper::Inf => {
                    if &rhs >= U::ZERO {
                        Ok(ValueWrapper::Inf)
                    } else {
                        Ok(ValueWrapper::NegInf)
                    }
                }
                ValueWrapper::NegInf => {
                    if &rhs >= U::ZERO {
                        Ok(ValueWrapper::NegInf)
                    } else {
                        Ok(ValueWrapper::Inf)
                    }
                }
            }
        }
    }
}

impl<T: Arithmetic, V: Arithmetic> Mul for ValueWrapper<T> where T: Mul<Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    fn mul(self, rhs: ValueWrapper<T>) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => Ok(ValueWrapper::from(lhs_value * rhs_value)),
                ValueWrapper::Inf => {
                    if &lhs_value >= T::ZERO {
                        Ok(ValueWrapper::Inf)
                    } else {
                        Ok(ValueWrapper::NegInf)
                    }
                }
                ValueWrapper::NegInf => {
                    if &lhs_value >= T::ZERO {
                        Ok(ValueWrapper::NegInf)
                    } else {
                        Ok(ValueWrapper::Inf)
                    }
                }
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Value(rhs_value) => {
                    if &rhs_value >= T::ZERO {
                        Ok(ValueWrapper::Inf)
                    } else {
                        Ok(ValueWrapper::NegInf)
                    }
                }
                ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::Value(rhs_value) => {
                    if &rhs_value >= T::ZERO {
                        Ok(ValueWrapper::NegInf)
                    } else {
                        Ok(ValueWrapper::Inf)
                    }
                }
                ValueWrapper::Inf => Ok(ValueWrapper::NegInf),
                ValueWrapper::NegInf => Ok(ValueWrapper::Inf),
            },
        }
    }
}

impl<T: Arithmetic, U: Arithmetic, V: Arithmetic> Mul<&U> for &ValueWrapper<T> where for<'a> &'a T: Mul<&'a U, Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    default fn mul(self, rhs: &U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value * rhs)),
            ValueWrapper::Inf => {
                if rhs >= U::ZERO {
                    Ok(ValueWrapper::Inf)
                } else {
                    Ok(ValueWrapper::NegInf)
                }
            }
            ValueWrapper::NegInf => {
                if rhs >= U::ZERO {
                    Ok(ValueWrapper::NegInf)
                } else {
                    Ok(ValueWrapper::Inf)
                }
            }
        }
    }
}

impl<T: Arithmetic, U: RealNumber, V: Arithmetic> Mul<&U> for &ValueWrapper<T> where for<'a> &'a T: Mul<&'a U, Output=V> {
    fn mul(self, rhs: &U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        if rhs.is_nan() {
            return Err(IllegalArgumentError {
                msg: String::from("Illegal argument NaN for value range!!!"),
            });
        }

        if rhs.is_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
            }
        } else if rhs.is_neg_inf() {
            match self {
                ValueWrapper::Value(_) | ValueWrapper::Inf => Ok(ValueWrapper::NegInf),
                ValueWrapper::NegInf => Ok(ValueWrapper::Inf),
            }
        } else {
            match self {
                ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value * rhs)),
                ValueWrapper::Inf => {
                    if rhs >= U::ZERO {
                        Ok(ValueWrapper::Inf)
                    } else {
                        Ok(ValueWrapper::NegInf)
                    }
                }
                ValueWrapper::NegInf => {
                    if rhs >= U::ZERO {
                        Ok(ValueWrapper::NegInf)
                    } else {
                        Ok(ValueWrapper::Inf)
                    }
                }
            }
        }
    }
}

impl<T: Arithmetic, V: Arithmetic> Mul for &ValueWrapper<T> where for<'a> &'a T: Mul<&'a T, Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    fn mul(self, rhs: &ValueWrapper<T>) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => Ok(ValueWrapper::from(lhs_value * rhs_value)),
                ValueWrapper::Inf => {
                    if lhs_value >= T::ZERO {
                        Ok(ValueWrapper::Inf)
                    } else {
                        Ok(ValueWrapper::NegInf)
                    }
                }
                ValueWrapper::NegInf => {
                    if lhs_value >= T::ZERO {
                        Ok(ValueWrapper::NegInf)
                    } else {
                        Ok(ValueWrapper::Inf)
                    }
                }
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Value(rhs_value) => {
                    if rhs_value >= T::ZERO {
                        Ok(ValueWrapper::Inf)
                    } else {
                        Ok(ValueWrapper::NegInf)
                    }
                }
                ValueWrapper::Inf => Ok(ValueWrapper::Inf),
                ValueWrapper::NegInf => Ok(ValueWrapper::NegInf),
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::Value(rhs_value) => {
                    if rhs >= T::ZERO {
                        Ok(ValueWrapper::NegInf)
                    } else {
                        Ok(ValueWrapper::Inf)
                    }
                }
                ValueWrapper::Inf => Ok(ValueWrapper::NegInf),
                ValueWrapper::NegInf => Ok(ValueWrapper::Inf),
            },
        }
    }
}

impl<T: Arithmetic, U: Arithmetic, V: Arithmetic> Div<U> for ValueWrapper<T> where T: Div<U, Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    default fn div(self, rhs: U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value / rhs)),
            ValueWrapper::Inf => {
                if &rhs >= U::ZERO {
                    Ok(ValueWrapper::Inf)
                } else {
                    Ok(ValueWrapper::NegInf)
                }
            }
            ValueWrapper::NegInf => {
                if &rhs >= U::ZERO {
                    Ok(ValueWrapper::NegInf)
                } else {
                    Ok(ValueWrapper::Inf)
                }
            }
        }
    }
}

impl<T: Arithmetic, U: RealNumber, V: Arithmetic> Div<U> for ValueWrapper<T> where T: Div<U, Output=V> {
    fn div(self, rhs: U) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        if rhs.is_nan() {
            return Err(IllegalArgumentError {
                msg: String::from("Illegal argument NaN for value range!!!"),
            });
        }

        if rhs.is_inf() {
            match self {
                ValueWrapper::Value(_) => Ok(ValueWrapper::from(V::ZERO.clone())),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid div between inf and inf!!!"),
                }),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid div between -inf and inf!!!"),
                }),
            }
        } else if rhs.is_neg_inf() {
            match self {
                ValueWrapper::Value(_) => Ok(ValueWrapper::from(V::ZERO.clone())),
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid div between inf and -inf!!!"),
                }),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid div between -inf and -inf!!!"),
                }),
            }
        } else {
            match self {
                ValueWrapper::Value(lhs_value) => Ok(ValueWrapper::from(lhs_value / rhs)),
                ValueWrapper::Inf => {
                    if &rhs >= U::ZERO {
                        Ok(ValueWrapper::Inf)
                    } else {
                        Ok(ValueWrapper::NegInf)
                    }
                }
                ValueWrapper::NegInf => {
                    if &rhs >= U::ZERO {
                        Ok(ValueWrapper::NegInf)
                    } else {
                        Ok(ValueWrapper::Inf)
                    }
                }
            }
        }
    }
}

impl<T: Arithmetic, V: Arithmetic> Div for ValueWrapper<T> where T: Div<Output=V> {
    type Output = Result<ValueWrapper<V>, IllegalArgumentError>;

    fn div(self, rhs: ValueWrapper<T>) -> Result<ValueWrapper<V>, IllegalArgumentError> {
        match self {
            ValueWrapper::Value(lhs_value) => match rhs {
                ValueWrapper::Value(rhs_value) => Ok(ValueWrapper::from(lhs_value / rhs_value)),
                ValueWrapper::Inf | ValueWrapper::NegInf => Ok(ValueWrapper::from(V::ZERO.clone())),
            },
            ValueWrapper::Inf => match rhs {
                ValueWrapper::Value(rhs_value) => {
                    if &rhs_value >= T::ZERO {
                        Ok(ValueWrapper::Inf)
                    } else {
                        Ok(ValueWrapper::NegInf)
                    }
                }
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid div between inf and inf!!!"),
                }),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid div between inf and -inf!!!"),
                }),
            },
            ValueWrapper::NegInf => match rhs {
                ValueWrapper::Value(rhs_value) => {
                    if &rhs_value >= T::ZERO {
                        Ok(ValueWrapper::NegInf)
                    } else {
                        Ok(ValueWrapper::Inf)
                    }
                }
                ValueWrapper::Inf => Err(IllegalArgumentError {
                    msg: String::from("Invalid div between -inf and inf!!!"),
                }),
                ValueWrapper::NegInf => Err(IllegalArgumentError {
                    msg: String::from("Invalid div between -inf and -inf!!!"),
                }),
            },
        }
    }
}
