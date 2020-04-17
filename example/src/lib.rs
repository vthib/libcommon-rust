use std::collections::HashMap;
use libcommon_ic::ic::{Channel, RpcRegister};
use libcommon_ic::types::Rpc;
use libcommon_ic::error;
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::sync::Mutex;

mod iop;
use iop::course::{CourseProgress, CourseType, StdCourseType, User};
use iop::course::rpcs::{user as rpc, custom as custom_rpc};
// needed to register and call rpcs with the right IOP module
use iop::course::modules::{course as course_mod};

// {{{ Helpers

fn std_course_get_nb_total_steps(typ: &StdCourseType) -> u32 {
    match typ {
        StdCourseType::C => 24,
        StdCourseType::PYTHON => 15,
        StdCourseType::RUST => 42,
    }
}

impl Default for User {
    fn default() -> User {
        User {
            id: 0,
            name: "".to_owned(),
            is_admin: false,
            email: None,
            courses: vec![]
        }
    }
}

impl PartialEq for CourseType {
    fn eq(&self, other: &Self) -> bool {
        match self {
            CourseType::Std(typ) => {
                match other {
                    CourseType::Std(otyp) => typ == otyp,
                    _ => false
                }
            },
            CourseType::CustomId(id) => {
                match other {
                    CourseType::CustomId(oid) => id == oid,
                    _ => false
                }
            },
        }
    }
}

// }}}
// {{{ User management

struct State {
    users: HashMap<u64, User>,
    next_id: u64,
}

lazy_static! {
    static ref STATE: Mutex<RefCell<State>> = Mutex::new(RefCell::new(State {
        users: HashMap::new(),
        next_id: 0,
    }));
}

impl State {
    fn find_user(&self, user_id: u64) -> Result<&User, error::Error> {
        match self.users.get(&user_id) {
            Some(u) => Ok(u),
            None => Err(error::Error::Generic(format!("unknown user {}", user_id))),
        }
    }

    fn create_user(&mut self, name: &str, email: Option<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let user = User {
            id,
            name: name.to_owned(),
            email,
            ..Default::default()
        };

        self.users.insert(id, user);
        id
    }

    fn set_user_progress(&mut self, user_id: u64, progress: CourseProgress) -> Result<(), error::Error> {
        match self.users.get_mut(&user_id) {
            Some(user) => {
                for course in user.courses.iter_mut() {
                    if course.r#type == progress.r#type {
                        course.completed_steps = progress.completed_steps;
                        return Ok(());
                    }
                }
                user.courses.push(progress.clone());
                Ok(())
            },
            None => Err(error::Error::Generic(format!("unknown user {}", user_id))),
        }
    }
}

// }}}
// {{{ User interface

async fn rpc_get_user(_ic: Channel, arg: rpc::GetArgs)
    -> Result<rpc::GetRes, error::Error>
{
    let state = STATE.lock().unwrap();
    let state = state.borrow();

    state.find_user(arg.id).map(|user| rpc::GetRes { user: user.clone() })
}

async fn rpc_set_progress(_ic: Channel, arg: rpc::SetProgressArgs)
    -> Result<rpc::SetProgressRes, error::Error>
{
    let state = STATE.lock().unwrap();
    let mut state = state.borrow_mut();

    state.set_user_progress(arg.id, arg.progress)
}

async fn rpc_get_completion_rate(mut ic: Channel, arg: rpc::GetCompletionRateArgs)
    -> Result<rpc::GetCompletionRateRes, error::Error>
{
    let state = STATE.lock().unwrap();
    let state = state.borrow();
    let user = state.find_user(arg.id)?;

    let mut done_steps = 0;
    let mut total_steps: u32 = 0;

    // naive way of waiting for multiple futures
    for course in &user.courses {
        done_steps += course.completed_steps;
        total_steps += match &course.r#type {
            CourseType::Std(t) => std_course_get_nb_total_steps(t),
            CourseType::CustomId(id) => {
                let args = custom_rpc::GetNbTotalStepsArgs { id: *id };
                let fut = custom_rpc::GetNbTotalSteps::call(&mut ic, course_mod::CUSTOM, args);
                fut.await?.nb_total_steps
            },
        };
    }

    let percent = if total_steps == 0 {
        0.
    } else if total_steps < done_steps {
        100.
    } else {
        let val = done_steps as f64 / total_steps as f64;
        // keep only two decimal digits of precision
        (val * 10000.).round() / 100.
    };

    Ok(rpc::GetCompletionRateRes { percent })
}

pub fn register_user_rpcs(reg: &mut RpcRegister) {
    // closure can be registered directly
    rpc::Create::implement(reg, course_mod::USER, |_ic, arg| async {
        let state = STATE.lock().unwrap();
        let mut state = state.borrow_mut();

        Ok(rpc::CreateRes { id: state.create_user(&arg.name, arg.email) })
    });

    // a top level function can be registered as well
    rpc::Get::implement(reg, course_mod::USER, rpc_get_user);
    rpc::SetProgress::implement(reg, course_mod::USER, rpc_set_progress);
    rpc::GetCompletionRate::implement(reg, course_mod::USER, rpc_get_completion_rate);
}

// }}}
// {{{ Custom interface

pub fn register_custom_rpcs(reg: &mut RpcRegister) {
    custom_rpc::GetNbTotalSteps::implement(reg, course_mod::CUSTOM, |_ic, arg| async move {
        let nb_total_steps = match arg.id {
            0 => 20,
            1 => 12,
            _ => {
                let err = format!("unknown custom course with id {}", arg.id);

                return Err(error::Error::Generic(err));
            }
        };
        Ok(custom_rpc::GetNbTotalStepsRes { nb_total_steps })
    });
}

// }}}

#[cfg(test)]
mod tests {
    use super::*;
    use libcommon_el;
    use libcommon_ic::ic::{Client, Server};
    use std::rc::Rc;

    async fn set_progress(ic: &mut Channel, id: u64, typ: CourseType, completed_steps: u32) {
        rpc::SetProgress::call(ic, course_mod::USER, rpc::SetProgressArgs {
            id,
            progress: CourseProgress {
                r#type: typ,
                completed_steps,
            }
        }).await.unwrap();
    }

    #[test]
    fn test_rpcs() {
        // require lib-common ic module for the whole test
        let _m = libcommon_ic::use_module();

        let mut server_reg = RpcRegister::new();
        register_user_rpcs(&mut server_reg);

        let mut client_reg = RpcRegister::new();
        register_custom_rpcs(&mut client_reg);

        libcommon_el::exec_test_async(async {
            // start server serving user rpcs
            let _server = Server::new("127.0.0.1", Some(server_reg));

            // start client serving custom rpcs
            let client_reg = Rc::new(client_reg);
            let mut client = Client::new(Some(&client_reg));

            // wait for both to be connected
            let connected = client.connect_once("127.0.0.1").await;
            assert!(connected);
            let mut ic = client.get_channel();

            // create two users
            let jojo_id = rpc::Create::call(&mut ic, course_mod::USER, rpc::CreateArgs {
                name: "Johnny Joestar".to_owned(),
                email: None,
            }).await.unwrap().id;

            let gyro_id = rpc::Create::call(&mut ic, course_mod::USER, rpc::CreateArgs {
                name: "Gyro Zeppeli".to_owned(),
                email: Some("gyro.z@napoli.it".to_owned()),
            }).await.unwrap().id;

            // Set progress for Jojo in one std course and 2 customs
            set_progress(&mut ic, jojo_id, CourseType::CustomId(1), 3).await;
            set_progress(&mut ic, jojo_id, CourseType::Std(StdCourseType::RUST), 10).await;
            set_progress(&mut ic, jojo_id, CourseType::CustomId(0), 18).await;

            // Set progress for Gyro in 2 std course and 1 custom
            set_progress(&mut ic, gyro_id, CourseType::Std(StdCourseType::PYTHON), 4).await;
            set_progress(&mut ic, gyro_id, CourseType::Std(StdCourseType::C), 8).await;
            set_progress(&mut ic, jojo_id, CourseType::CustomId(1), 12).await;

            // Check completion rate for both users
            let rate = rpc::GetCompletionRate::call(
                &mut ic,
                course_mod::USER,
                rpc::GetCompletionRateArgs { id: jojo_id }
            ).await.unwrap().percent;
            assert_eq!(rate, 54.05);

            let rate = rpc::GetCompletionRate::call(
                &mut ic,
                course_mod::USER,
                rpc::GetCompletionRateArgs { id: gyro_id }
            ).await.unwrap().percent;
            assert_eq!(rate, 30.77);
        });
    }
}
