#include "pid_ctrl.h"
#include "types.h"

// input buffer
uint8_t INPUT[8];

// output buffer
uint8_t OUTPUT[4];

static pidctl_t pid;

void process() {
  // read inputs
  float current_temperature = *INPUT;
  float set_temperature = INPUT[4];

  // calculate error
  float error = current_temperature - set_temperature;

  // calculate setpoint
  float setpoint;
  pidctl(pid, error, setpoint);

  // write output
  *OUTPUT = setpoint;
}
