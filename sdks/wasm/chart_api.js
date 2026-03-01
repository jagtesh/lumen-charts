// Lightweight Charts API Wrapper for WASM

import init, * as wasm from './chart_wasm.js';

export async function createChart(containerOrElement) {
    if (!navigator.gpu) {
        throw new Error("WebGPU not supported in this browser.");
    }
    await init();
    await wasm.chart_start();

    // The Rust code creates its own canvas named 'chart-canvas' 
    // Wait for the rust-generated canvas to exist
    const canvas = document.getElementById('chart-canvas');
    if (canvas) {
        wireInteractions(canvas, wasm);
    }

    return new ChartAPI(wasm);
}

class ChartAPI {
    constructor(wasmModule) {
        this.wasm = wasmModule;
    }
    addOhlcSeries(options) {
        return new SeriesAPI(this.wasm, this.wasm.chart_add_ohlc_series, options, 'ohlc');
    }
    addCandlestickSeries(options) {
        return new SeriesAPI(this.wasm, this.wasm.chart_add_candlestick_series, options, 'ohlc');
    }
    addLineSeries(options) {
        return new SeriesAPI(this.wasm, this.wasm.chart_add_line_series, options);
    }
    addAreaSeries(options) {
        return new SeriesAPI(this.wasm, this.wasm.chart_add_area_series, options);
    }
    addHistogramSeries(options) {
        return new SeriesAPI(this.wasm, this.wasm.chart_add_histogram_series, options);
    }
    addBaselineSeries(options, baseValue) {
        // Special case for base_value parameter
        const internalAdd = (data) => this.wasm.chart_add_baseline_series(data, baseValue || 0.0);
        return new SeriesAPI(this.wasm, internalAdd, options);
    }

    /**
     * Set the primary series data (OHLC bars).
     * Accepts an array of {time, open, high, low, close} objects.
     */
    setData(data) {
        if (!Array.isArray(data)) {
            throw new TypeError('chart.setData(): expected an array of {time, open, high, low, close} objects');
        }
        if (data.length > 0) {
            const d = data[0];
            if (d.time === undefined) throw new TypeError('chart.setData(): each item must have a "time" field');
            if (d.open === undefined || d.high === undefined || d.low === undefined || d.close === undefined) {
                throw new TypeError('chart.setData(): each item must have "open", "high", "low", "close" fields');
            }
            if (d.value !== undefined) {
                console.warn('chart.setData(): "value" field is ignored for primary OHLC data — use addLineSeries().setData() for line data');
            }
        }
        const flat = new Float64Array(data.length * 5);
        for (let i = 0; i < data.length; i++) {
            flat[i * 5] = data[i].time;
            flat[i * 5 + 1] = data[i].open;
            flat[i * 5 + 2] = data[i].high;
            flat[i * 5 + 3] = data[i].low;
            flat[i * 5 + 4] = data[i].close;
        }
        this.wasm.chart_set_data(flat);
        this.wasm.chart_render_if_needed();
    }

    /**
     * Change the rendering type of the primary series.
     * type: 'ohlc' | 'candlestick' | 'line' | 'area' | 'histogram' | 'baseline'
     */
    setSeriesType(type) {
        const typeMap = { 'ohlc': 0, 'candlestick': 1, 'line': 2, 'area': 3, 'histogram': 4, 'baseline': 5 };
        const code = typeMap[type];
        if (code === undefined) {
            throw new Error(`chart.setSeriesType(): unknown type "${type}". Valid: ${Object.keys(typeMap).join(', ')}`);
        }
        this.wasm.chart_set_series_type(code);
        this.wasm.chart_render_if_needed();
    }

    removeSeries(series) {
        this.wasm.chart_remove_series(series.id);
    }

    applyOptions(options) {
        this.wasm.chart_apply_options(JSON.stringify(options));
        this.wasm.chart_render_if_needed();
    }

    addPane(heightStretch) {
        const id = this.wasm.chart_add_pane(heightStretch);
        return new PaneAPI(id);
    }

    removePane(pane) {
        this.wasm.chart_remove_pane(pane.id);
    }

    timeScale() {
        return new TimeScaleAPI(this.wasm);
    }

    priceScale() {
        return new PriceScaleAPI(this.wasm);
    }

    fitContent() {
        this.wasm.chart_fit_content();
        this.wasm.chart_render_if_needed();
    }
}

class PaneAPI {
    constructor(id) {
        this.id = id;
    }
}

class SeriesAPI {
    constructor(wasmModule, wasmAddFn, options, seriesKind) {
        this.wasm = wasmModule;
        this.wasmAddFn = wasmAddFn;
        this.id = null;
        this.options = options;
        this.seriesKind = seriesKind || 'line'; // 'ohlc' or 'line'
        this._pendingPane = null; // deferred pane assignment
    }

    setData(data) {
        if (!Array.isArray(data)) {
            throw new TypeError('series.setData(): expected an array of data objects');
        }

        let flatData;

        if (data.length > 0) {
            const d = data[0];
            if (d.time === undefined) {
                throw new TypeError('series.setData(): each item must have a "time" field');
            }

            if (this.seriesKind === 'ohlc') {
                // OHLC / Candle series
                if (d.open === undefined || d.high === undefined || d.low === undefined || d.close === undefined) {
                    throw new TypeError('series.setData(): OHLC series requires "open", "high", "low", "close" fields');
                }
                flatData = new Float64Array(data.length * 5);
                for (let i = 0; i < data.length; i++) {
                    flatData[i * 5] = data[i].time;
                    flatData[i * 5 + 1] = data[i].open;
                    flatData[i * 5 + 2] = data[i].high;
                    flatData[i * 5 + 3] = data[i].low;
                    flatData[i * 5 + 4] = data[i].close;
                }
            } else {
                // Line / Area / Histogram / Baseline
                if (d.value === undefined) {
                    if (d.close !== undefined) {
                        console.warn('series.setData(): line/area/histogram series expects {time, value}. Using "close" as "value".');
                        data = data.map(item => ({ time: item.time, value: item.close }));
                    } else {
                        throw new TypeError('series.setData(): each item must have a "value" field (or "close" for OHLC series)');
                    }
                }
                flatData = new Float64Array(data.length * 2);
                for (let i = 0; i < data.length; i++) {
                    flatData[i * 2] = data[i].time;
                    flatData[i * 2 + 1] = data[i].value;
                }
            }
        } else {
            flatData = new Float64Array(0);
        }

        if (this.id !== null) {
            this.wasm.chart_remove_series(this.id);
        }

        this.id = this.wasmAddFn(flatData);

        if (this.options) {
            this.wasm.chart_series_apply_options(this.id, JSON.stringify(this.options));
        }

        // Apply deferred pane assignment
        if (this._pendingPane !== null) {
            this.wasm.chart_series_move_to_pane(this.id, this._pendingPane.id);
            this._pendingPane = null;
        }

        this.wasm.chart_render_if_needed();
    }

    applyOptions(options) {
        if (!this.options) this.options = {};
        Object.assign(this.options, options);
        if (this.id !== null) {
            this.wasm.chart_series_apply_options(this.id, JSON.stringify(options));
            this.wasm.chart_render_if_needed();
        }
    }

    moveToPane(pane) {
        if (this.id !== null) {
            this.wasm.chart_series_move_to_pane(this.id, pane.id);
            this.wasm.chart_render_if_needed();
        } else {
            // Series not created yet — defer until setData() is called
            this._pendingPane = pane;
        }
    }
}

class TimeScaleAPI {
    constructor(wasmModule) { this.wasm = wasmModule; }
    scrollToRealTime() {
        this.wasm.chart_time_scale_scroll_to_real_time();
        this.wasm.chart_render_if_needed();
    }
    scrollToPosition(pos) {
        this.wasm.chart_time_scale_scroll_to_position(pos);
        this.wasm.chart_render_if_needed();
    }
    reset() {
        this.wasm.chart_time_scale_reset();
        this.wasm.chart_render_if_needed();
    }
}

class PriceScaleAPI {
    constructor(wasmModule) { this.wasm = wasmModule; }
    setMode(mode) {
        this.wasm.chart_price_scale_set_mode(mode);
        this.wasm.chart_render_if_needed();
    }
}

function wireInteractions(canvas, wasm) {
    const rect = () => canvas.getBoundingClientRect();

    function toChart(e) {
        const r = rect();
        return { x: e.clientX - r.left, y: e.clientY - r.top };
    }

    canvas.addEventListener('mousemove', (e) => {
        const p = toChart(e);
        wasm.chart_pointer_move(p.x, p.y);
    });

    canvas.addEventListener('mousedown', (e) => {
        const p = toChart(e);
        wasm.chart_pointer_down(p.x, p.y);
    });

    canvas.addEventListener('mouseup', (e) => {
        const p = toChart(e);
        wasm.chart_pointer_up(p.x, p.y);
    });

    canvas.addEventListener('mouseleave', () => {
        wasm.chart_pointer_leave();
    });

    canvas.addEventListener('wheel', (e) => {
        e.preventDefault();
        const p = toChart(e);
        if (e.ctrlKey || e.metaKey) {
            const factor = 1.0 - e.deltaY * 0.005;
            wasm.chart_zoom(factor, p.x);
        } else {
            wasm.chart_scroll(-e.deltaX, e.deltaY);
        }
    }, { passive: false });

    document.addEventListener('keydown', (e) => {
        const keyMap = {
            'ArrowLeft': 37, 'ArrowRight': 39,
            'ArrowUp': 38, 'ArrowDown': 40,
            '+': 187, '=': 187, '-': 189,
            'Home': 36, 'End': 35,
        };
        const code = keyMap[e.key];
        if (code !== undefined) {
            wasm.chart_key_down(code);
        }
    });

    function frameTick() {
        wasm.chart_tick();
        wasm.chart_render_if_needed();
        requestAnimationFrame(frameTick);
    }
    requestAnimationFrame(frameTick);
}
