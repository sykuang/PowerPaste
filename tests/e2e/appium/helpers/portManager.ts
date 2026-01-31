/**
 * Port manager for parallel Appium test execution.
 *
 * Allocates unique ports per worker to prevent collisions when
 * running multiple test files in parallel.
 */

import { getPort } from 'get-port-please';
import { config as ciConfig } from './ciConfig.js';

// Track allocated ports to avoid collisions
const allocatedPorts = new Set<number>();

// Base port ranges (higher in CI to avoid conflicts with other services)
const BASE_PORTS = {
  appium: ciConfig.isCI ? 5723 : 4723,
  macSystem: ciConfig.isCI ? 11100 : 10100,
  macWda: ciConfig.isCI ? 9100 : 8100,
  windowsSystem: ciConfig.isCI ? 5724 : 4724,
  devtools: ciConfig.isCI ? 10222 : 9222,
};

/**
 * Allocate a unique port for a service.
 */
export async function allocatePort(
  service: keyof typeof BASE_PORTS,
  workerId: number = 0
): Promise<number> {
  const basePort = BASE_PORTS[service] + workerId;

  // Try the expected port first
  const port = await getPort({ port: basePort, portRange: [basePort, basePort + 100] });

  if (allocatedPorts.has(port)) {
    // Find an alternative port
    const altPort = await getPort({ portRange: [basePort + 100, basePort + 200] });
    allocatedPorts.add(altPort);
    logPortAllocation(service, workerId, altPort, 'fallback');
    return altPort;
  }

  allocatedPorts.add(port);
  logPortAllocation(service, workerId, port, 'primary');
  return port;
}

/**
 * Get static port for a service (without checking availability).
 * Use this when you need deterministic ports.
 */
export function getStaticPort(service: keyof typeof BASE_PORTS, workerId: number = 0): number {
  return BASE_PORTS[service] + workerId;
}

/**
 * Release a port when done.
 */
export function releasePort(port: number): void {
  allocatedPorts.delete(port);
  if (ciConfig.isCI) {
    console.log(`[port-manager] Released port ${port}`);
  }
}

/**
 * Release all allocated ports.
 */
export function releaseAllPorts(): void {
  const count = allocatedPorts.size;
  allocatedPorts.clear();
  if (ciConfig.isCI) {
    console.log(`[port-manager] Released all ${count} ports`);
  }
}

function logPortAllocation(
  service: string,
  workerId: number,
  port: number,
  type: 'primary' | 'fallback'
): void {
  if (ciConfig.isCI) {
    console.log(
      `[port-manager] Allocated ${service} port ${port} for worker ${workerId} (${type})`
    );
  }
}

/**
 * Get all ports needed for a worker.
 */
export async function allocateWorkerPorts(workerId: number = 0): Promise<{
  appium: number;
  macSystem: number;
  macWda: number;
  windowsSystem: number;
  devtools: number;
}> {
  const [appium, macSystem, macWda, windowsSystem, devtools] = await Promise.all([
    allocatePort('appium', workerId),
    allocatePort('macSystem', workerId),
    allocatePort('macWda', workerId),
    allocatePort('windowsSystem', workerId),
    allocatePort('devtools', workerId),
  ]);

  return { appium, macSystem, macWda, windowsSystem, devtools };
}
