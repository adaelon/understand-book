// Ideal horizontal mass–spring oscillator. SI units; no damping or friction.
export function springModel(s) {
  const phase=2*Math.PI*(s.semantic_state+s.transition_progress),omega=Math.sqrt(s.stiffness/s.mass);
  const x=s.amplitude*Math.cos(phase),velocity=-s.amplitude*omega*Math.sin(phase);
  return {phase,omega,x,velocity,force:-s.stiffness*x,period:2*Math.PI/omega,
    time:(s.semantic_state+s.transition_progress)*2*Math.PI/omega,
    potential:.5*s.stiffness*x*x,kinetic:.5*s.mass*velocity*velocity,total:.5*s.stiffness*s.amplitude*s.amplitude};
}
