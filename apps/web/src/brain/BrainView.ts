import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { activationColor, rateAt, type ActivationFrame } from './activation';
import { loadBrain, type BrainData } from './geometry';
import { createBrainLegend } from './BrainLegend';

export class BrainView {
  private data?: BrainData;
  private frame?: ActivationFrame;
  private renderer?: THREE.WebGLRenderer;
  private controls?: OrbitControls;
  private geometry?: THREE.BufferGeometry;
  private points?: THREE.Points;
  private marker?: THREE.Points;
  private scene = new THREE.Scene();
  private camera = new THREE.PerspectiveCamera(40, 1, 0.01, 100);
  private observer?: ResizeObserver;
  private selected: number | null = null;
  private disposed = false;
  private classFilter = '';
  private regionFilter = '';
  private filtered: number[] = [];
  private colors?: Float32Array;
  private viewport: HTMLElement;
  private info: HTMLElement;
  private status: HTMLElement;
  private top: HTMLElement;
  private inspection: HTMLElement;
  private filters: HTMLElement;

  constructor(private host: HTMLElement, private onExpansionChange: (expanded: boolean) => void = () => {}) {
    host.innerHTML = `<div class="brain-toolbar"><div><h2 tabindex="-1">Inside the fly</h2><span class="eyebrow">MaleCNS · real soma locations</span></div><button type="button" class="quiet expand-brain" aria-expanded="false" aria-label="Expand brain view">⤢</button></div>
      <div class="brain-viewport" aria-label="Interactive MaleCNS soma point cloud"></div>
      <p class="brain-status" role="status">Loading real neuron geometry…</p>
      <div class="brain-filters"></div><div class="brain-info"></div>
      <details class="neuron-details"><summary>Inspect neurons</summary><p class="inspection">Click a soma or select an active neuron.</p><div class="top-neurons"></div></details>`;
    this.viewport = host.querySelector('.brain-viewport')!;
    this.status = host.querySelector('.brain-status')!;
    this.info = host.querySelector('.brain-info')!;
    this.filters = host.querySelector('.brain-filters')!;
    this.top = host.querySelector('.top-neurons')!;
    this.inspection = host.querySelector('.inspection')!;
    this.viewport.after(createBrainLegend());
    host.querySelector<HTMLButtonElement>('.expand-brain')!.onclick = () => this.setExpanded(!host.classList.contains('expanded'));
    host.addEventListener('keydown', event => {
      if (event.key === 'Escape' && host.classList.contains('expanded')) host.querySelector<HTMLButtonElement>('.expand-brain')!.click();
    });
    void this.initialize();
  }

  setExpanded(expanded: boolean, focusHeading = false): void {
    this.host.classList.toggle('expanded', expanded);
    const button = this.host.querySelector<HTMLButtonElement>('.expand-brain')!;
    button.setAttribute('aria-expanded', String(expanded));
    button.setAttribute('aria-label', expanded ? 'Collapse brain view' : 'Expand brain view');
    button.textContent = expanded ? '×' : '⤢';
    this.onExpansionChange(expanded);
    if (expanded && focusHeading) requestAnimationFrame(() => this.host.querySelector<HTMLElement>('h2')!.focus({ preventScroll: true }));
  }

  private async initialize(): Promise<void> {
    try {
      const data = await loadBrain();
      if (this.disposed) return;
      this.data = data;
      const positions = data.positions.slice();
      const bounds = new THREE.Box3().setFromArray(positions);
      const center = bounds.getCenter(new THREE.Vector3());
      const size = bounds.getSize(new THREE.Vector3());
      const scale = 2 / Math.max(size.x, size.y, size.z);
      for (let i = 0; i < positions.length; i += 3) {
        positions[i] = (positions[i] - center.x) * scale;
        positions[i + 1] = -(positions[i + 1] - center.y) * scale;
        positions[i + 2] = (positions[i + 2] - center.z) * scale;
      }
      this.geometry = new THREE.BufferGeometry();
      this.geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      this.geometry.setAttribute('denseIndex', new THREE.BufferAttribute(data.denseIndices, 1));
      this.colors = new Float32Array(positions.length);
      this.geometry.setAttribute('color', new THREE.BufferAttribute(this.colors, 3).setUsage(THREE.DynamicDrawUsage));
      this.points = new THREE.Points(this.geometry, new THREE.PointsMaterial({ size: 0.008, vertexColors: true, transparent: true, opacity: 0.9 }));
      this.scene.add(this.points);
      const markerGeometry = new THREE.BufferGeometry();
      markerGeometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(3), 3));
      this.marker = new THREE.Points(markerGeometry, new THREE.PointsMaterial({ size: 0.036, color: '#ffffff', depthTest: false }));
      this.marker.visible = false;
      this.scene.add(this.marker);
      this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
      this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
      this.viewport.append(this.renderer.domElement);
      this.renderer.domElement.setAttribute('aria-label', 'Rotate by dragging; zoom with scroll. Neuron inspection is also available below.');
      this.renderer.domElement.addEventListener('webglcontextlost', event => { event.preventDefault(); this.status.textContent = '3D graphics context lost. Reload the page to restore the brain view.'; });
      this.camera.position.set(0, 0, 2.85);
      this.controls = new OrbitControls(this.camera, this.renderer.domElement);
      this.controls.minDistance = 0.6;
      this.controls.maxDistance = 7;
      this.controls.addEventListener('change', () => this.draw());
      let down = { x: 0, y: 0 };
      this.renderer.domElement.addEventListener('pointerdown', event => { down = { x: event.clientX, y: event.clientY }; });
      this.renderer.domElement.addEventListener('pointerup', event => {
        if (Math.hypot(event.clientX - down.x, event.clientY - down.y) > 5) return;
        const rect = this.renderer!.domElement.getBoundingClientRect();
        const ray = new THREE.Raycaster();
        ray.params.Points.threshold = 0.012;
        ray.setFromCamera(new THREE.Vector2((event.clientX - rect.left) / rect.width * 2 - 1, -(event.clientY - rect.top) / rect.height * 2 + 1), this.camera);
        const hit = ray.intersectObject(this.points!)[0];
        if (hit?.index !== undefined) this.select(data.denseIndices[hit.index]);
      });
      this.observer = new ResizeObserver(() => {
        const { width, height } = this.viewport.getBoundingClientRect();
        if (!width || !height) return;
        this.camera.aspect = width / height;
        this.camera.updateProjectionMatrix();
        this.renderer!.setSize(width, height, false);
        this.draw();
      });
      this.observer.observe(this.viewport);
      this.addFilter('Neuron superclass', 2, value => { this.classFilter = value; });
      this.addFilter('Soma neuromere', 3, value => { this.regionFilter = value; });
      this.applyFilter();
      this.update(this.frame);
    } catch (error) {
      this.status.textContent = `Brain view unavailable: ${error instanceof Error ? error.message : String(error)}. Chess remains available.`;
    }
  }

  private addFilter(label: string, column: 2 | 3, set: (value: string) => void): void {
    const wrapper = document.createElement('label');
    wrapper.textContent = label;
    const select = document.createElement('select');
    select.setAttribute('aria-label', label);
    select.add(new Option('All', ''));
    const values = [...new Set(this.data!.annotations.map(row => row[column] || 'Unannotated'))].sort();
    values.forEach(value => select.add(new Option(value, value)));
    select.onchange = () => { set(select.value); this.applyFilter(); this.update(this.frame); };
    wrapper.append(select); this.filters.append(wrapper);
  }

  private applyFilter(): void {
    if (!this.data || !this.geometry) return;
    this.filtered = [];
    this.data.denseIndices.forEach((denseIndex, vertex) => {
      const row = this.data!.annotations[denseIndex];
      if ((!this.classFilter || (row[2] || 'Unannotated') === this.classFilter) && (!this.regionFilter || (row[3] || 'Unannotated') === this.regionFilter)) this.filtered.push(vertex);
    });
    this.geometry.setIndex(this.filtered);
    this.info.textContent = `${this.filtered.length.toLocaleString()} visible · ${this.data.denseIndices.length.toLocaleString()} with soma geometry · ${this.data.bodyIds.length.toLocaleString()} simulated. Missing somas remain in the simulation.`;
  }

  update(frame?: ActivationFrame): void {
    this.frame = frame;
    if (!this.data || !this.geometry || !this.colors) return;
    this.data.denseIndices.forEach((index, vertex) => this.colors!.set(activationColor(rateAt(frame?.neuronRates, index)), vertex * 3));
    this.geometry.getAttribute('color').needsUpdate = true;
    const selectedRates = this.filtered.reduce((sum, vertex) => sum + rateAt(frame?.neuronRates, this.data!.denseIndices[vertex]), 0);
    this.status.textContent = frame
      ? `Step ${frame.step} · ${frame.tMs.toFixed(0)} ms simulated · ${(frame.elapsedMs / 1000).toFixed(1)} s elapsed · ${frame.backend}. Visible mean ≈ ${(selectedRates / Math.max(1, this.filtered.length)).toFixed(2)}.`
      : 'No activity sampled yet. Drag to rotate · scroll to zoom.';
    const region = frame?.regionRates.find(item => item.region === this.regionFilter);
    if (region) this.status.textContent += ` Neuromere mean: ${region.rate.toFixed(2)} across ${region.neuronCount.toLocaleString()} simulated neurons.`;
    this.top.replaceChildren();
    for (const item of frame?.topNeurons ?? []) {
      const button = document.createElement('button');
      button.className = 'neuron-chip';
      button.textContent = `${this.data.bodyIds[item.denseIndex]} · ${item.rate.toFixed(2)}`;
      button.onclick = () => this.select(item.denseIndex);
      this.top.append(button);
    }
    this.updateInspection();
    this.draw();
  }

  private select(index: number): void {
    this.selected = index;
    this.host.querySelector<HTMLDetailsElement>('.neuron-details')!.open = true;
    this.updateInspection(); this.draw();
  }
  private updateInspection(): void {
    if (!this.data || this.selected === null) return;
    const index = this.selected;
    const row = this.data.annotations[index];
    const exact = this.frame?.topNeurons.find(item => item.denseIndex === index)?.rate;
    const rate = exact ?? rateAt(this.frame?.neuronRates, index);
    this.inspection.textContent = `Body ${this.data.bodyIds[index]} · dense ${index}\nType: ${row[0] || 'unannotated'} · class: ${row[1] || 'unannotated'}\nSuperclass: ${row[2] || 'unannotated'} · soma neuromere: ${row[3] || 'unannotated'}\nRate: ${exact === undefined ? '≈ ' : ''}${rate.toFixed(3)}${this.frame ? '' : ' (not sampled)'}\nOutput group: ${this.data.outputs.get(index)?.join(', ') || 'none'}`;
    const vertex = this.data.denseIndices.indexOf(index);
    if (this.marker && this.geometry) {
      this.marker.visible = vertex >= 0 && this.filtered.includes(vertex);
      if (vertex >= 0) {
        const position = this.geometry.getAttribute('position');
        this.marker.geometry.getAttribute('position').setXYZ(0, position.getX(vertex), position.getY(vertex), position.getZ(vertex));
        this.marker.geometry.getAttribute('position').needsUpdate = true;
      } else this.inspection.textContent += '\nNo recorded soma position; neuron is still simulated.';
    }
  }
  reset(): void { this.selected = null; if (this.marker) this.marker.visible = false; this.inspection.textContent = 'Click a soma or select an active neuron.'; this.update(); }
  private draw(): void { this.renderer?.render(this.scene, this.camera); }
  dispose(): void {
    this.disposed = true;
    this.observer?.disconnect(); this.controls?.dispose();
    for (const object of [this.points, this.marker]) {
      object?.geometry.dispose();
      if (object && !Array.isArray(object.material)) object.material.dispose();
    }
    this.renderer?.dispose();
  }
}
