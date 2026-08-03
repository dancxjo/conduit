import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.atomic.AtomicLong;
import reactor.core.publisher.Flux;
import reactor.core.scheduler.Schedulers;

public final class ComparativeBenchmark {
    private ComparativeBenchmark() {}

    private static String argument(String[] args, String name, String fallback) {
        for (int index = 0; index + 1 < args.length; index++) {
            if (args[index].equals("--" + name)) return args[index + 1];
        }
        return fallback;
    }

    private static boolean flag(String[] args, String name) {
        for (String argument : args) {
            if (argument.equals("--" + name)) return true;
        }
        return false;
    }

    private static String nullableLong(Long value) {
        return value == null ? "null" : Long.toString(value);
    }

    private static String latencyJson(List<Long> values) {
        StringBuilder result = new StringBuilder("[");
        for (int index = 0; index < values.size(); index++) {
            if (index > 0) result.append(',');
            result.append(values.get(index));
        }
        return result.append(']').toString();
    }

    private static Long procStatusBytes(String field) {
        try {
            for (String line : Files.readAllLines(Path.of("/proc/self/status"))) {
                if (line.startsWith(field)) {
                    return Long.parseLong(line.trim().split("\\s+")[1]) * 1024L;
                }
            }
        } catch (Exception ignored) {
            // The field remains explicitly unavailable on non-Linux hosts.
        }
        return null;
    }

    private static void runIdentitySample(String[] args, String sampleKind, int trial, String thermalState) {
        String workload = argument(args, "workload", "map");
        int operators = Integer.parseInt(argument(args, "operators", "1"));
        int values = Integer.parseInt(argument(args, "values", "1000000"));
        int queueItems = Integer.parseInt(argument(args, "queue-items", "256"));
        int stride = Integer.parseInt(argument(args, "latency-sample-stride", "1024"));
        if (workload.equals("bounded-async")) {
            throw new IllegalArgumentException("an identity loop cannot model a bounded asynchronous boundary");
        }
        long assemblyStarted = System.nanoTime();
        long[] starts = new long[(values + stride - 1) / stride];
        List<Long> latencies = new ArrayList<>(starts.length);
        int transformCount = workload.equals("merge") ? Math.max(0, operators - 1) : operators;
        long assemblyNs = System.nanoTime() - assemblyStarted;
        Long residentBefore = procStatusBytes("VmRSS:");
        com.sun.management.OperatingSystemMXBean operatingSystem =
            (com.sun.management.OperatingSystemMXBean) ManagementFactory.getOperatingSystemMXBean();
        long cpuBefore = operatingSystem.getProcessCpuTime();
        long acceptedValues = 0;
        long usefulOutputs = 0;
        long checksum = 0;
        long steadyStarted = System.nanoTime();
        for (int original = 0; original < values; original++) {
            acceptedValues++;
            if (original % stride == 0) starts[original / stride] = System.nanoTime();
            int value = original;
            boolean retained = true;
            for (int index = 0; index < transformCount; index++) {
                if (workload.equals("map-filter") && index % 2 == 1) {
                    retained = value % 2 == 0;
                    if (!retained) break;
                } else {
                    value += 2;
                }
            }
            if (retained) {
                usefulOutputs++;
                checksum += value;
                if (original % stride == 0) latencies.add(System.nanoTime() - starts[original / stride]);
            }
        }
        long steadyNs = System.nanoTime() - steadyStarted;
        long cpuNs = operatingSystem.getProcessCpuTime() - cpuBefore;
        Long residentAfter = procStatusBytes("VmRSS:");
        Long residentPeak = procStatusBytes("VmHWM:");
        if (checksum == Long.MIN_VALUE) throw new IllegalStateException("unreachable identity checksum");
        System.out.printf(Locale.ROOT,
            "{\"schema\":\"conduit.comparative-raw-sample\",\"schema_version\":0,\"fixture_revision\":0," +
            "\"runtime\":{\"id\":\"java-identity-loop\",\"comparison_role\":\"language-lower-bound\",\"version\":\"%s\",\"execution_mode\":\"single-threaded-for-loop\",\"build_profile\":\"JVM default\",\"scheduler\":\"none\",\"fusion\":\"not-applicable\",\"batching\":\"none\",\"concurrency\":1}," +
            "\"workload\":{\"id\":\"%s\",\"operators\":%d,\"input_values\":%d,\"queue_capacity_items\":0,\"ordering\":\"ascending input order; merge boundary omitted\",\"pressure\":\"not-applicable\",\"terminal\":\"loop exhaustion\",\"loss\":\"none\",\"slow_consumer_yields\":0,\"recovery_after_outputs\":0}," +
            "\"exact_identity\":{\"logical_fixture\":\"comparative-local-depth/%s/%d/%d/%d/%d\",\"plan_identity\":null,\"source_semantic_hash\":null,\"artifact_digest\":null}," +
            "\"sample_kind\":\"%s\",\"trial\":%d,\"thermal_state\":\"%s\",\"phases\":{\"assembly_ns\":%d,\"plan_seal_ns\":null,\"start_ns\":null,\"steady_ns\":%d,\"pressure_ns\":null,\"recovery_ns\":null}," +
            "\"process_cpu_ns\":%d,\"outcomes\":{\"offered\":%d,\"admitted\":%d,\"completed_useful\":%d,\"rejected\":0,\"sampled\":0,\"coalesced\":0,\"dropped\":0,\"retried\":0,\"terminal\":1}," +
            "\"allocations\":{\"scope\":\"unavailable-without-JVM-agent\",\"calls\":null,\"bytes\":null}," +
            "\"memory\":{\"resident_before_bytes\":%s,\"resident_after_bytes\":%s,\"resident_peak_bytes\":%s,\"planned_memory_bytes\":null,\"executor_overhead_bytes\":null,\"queue_items_high_water\":null,\"queue_payload_bytes_high_water\":null,\"ready_slots_high_water\":null,\"evidence_slots_high_water\":null}," +
            "\"latency\":{\"clock\":\"System.nanoTime monotonic\",\"sample_stride\":%d,\"samples_ns\":%s}," +
            "\"semantic_notes\":[\"This no-framework Java loop is a language-cost lower bound, not a reactive-runtime competitor.\",\"It has no subscription, scheduler, demand, queue, evidence, or merge boundary and cannot support runtime claims.\"]}%n",
            System.getProperty("java.version"), workload, operators, values,
            workload, operators, values, queueItems, stride, sampleKind, trial, thermalState,
            assemblyNs, steadyNs, cpuNs, values, acceptedValues, usefulOutputs, nullableLong(residentBefore),
            nullableLong(residentAfter), nullableLong(residentPeak), stride, latencyJson(latencies));
    }

    private static void runSample(String[] args, String sampleKind, int trial, String thermalState) {
        String workload = argument(args, "workload", "map");
        int operators = Integer.parseInt(argument(args, "operators", "1"));
        int values = Integer.parseInt(argument(args, "values", "1000000"));
        int queueItems = Integer.parseInt(argument(args, "queue-items", "256"));
        int stride = Integer.parseInt(argument(args, "latency-sample-stride", "1024"));
        if (operators < 1 || values < 1 || queueItems < 1 || stride < 1) {
            throw new IllegalArgumentException("operators, values, queue items, and stride must be positive");
        }

        long[] starts = new long[(values + stride - 1) / stride];
        List<Long> latencies = new ArrayList<>(starts.length);
        AtomicLong acceptedValues = new AtomicLong();
        long assemblyStarted = System.nanoTime();
        int split = values / 2;
        Flux<Integer> source;
        int transformCount = operators;
        if (workload.equals("merge")) {
            Flux<Integer> left = Flux.range(0, split).doOnNext(value -> {
                acceptedValues.incrementAndGet();
                if (value % stride == 0) starts[value / stride] = System.nanoTime();
            });
            Flux<Integer> right = Flux.range(split, values - split).doOnNext(value -> {
                acceptedValues.incrementAndGet();
                if (value % stride == 0) starts[value / stride] = System.nanoTime();
            });
            source = Flux.merge(left, right);
            transformCount = Math.max(0, operators - 1);
        } else {
            source = Flux.range(0, values).doOnNext(value -> {
                acceptedValues.incrementAndGet();
                if (value % stride == 0) starts[value / stride] = System.nanoTime();
            });
        }
        Flux<Integer> pipeline = source;
        for (int index = 0; index < transformCount; index++) {
            if (workload.equals("map-filter") && index % 2 == 1) {
                pipeline = pipeline.filter(value -> value % 2 == 0);
            } else {
                pipeline = pipeline.map(value -> value + 2);
            }
        }
        if (workload.equals("bounded-async")) {
            pipeline = pipeline.publishOn(Schedulers.single(), queueItems);
        }
        long assemblyNs = System.nanoTime() - assemblyStarted;

        Long residentBefore = procStatusBytes("VmRSS:");
        com.sun.management.OperatingSystemMXBean operatingSystem =
            (com.sun.management.OperatingSystemMXBean) ManagementFactory.getOperatingSystemMXBean();
        long cpuBefore = operatingSystem.getProcessCpuTime();
        long steadyStarted = System.nanoTime();
        int finalTransformCount = transformCount;
        long usefulOutputs = pipeline.doOnNext(value -> {
            int original = value - (2 * finalTransformCount);
            if (original >= 0 && original % stride == 0 && starts[original / stride] != 0) {
                latencies.add(System.nanoTime() - starts[original / stride]);
            }
        }).count().block();
        long steadyNs = System.nanoTime() - steadyStarted;
        long cpuNs = operatingSystem.getProcessCpuTime() - cpuBefore;
        Long residentAfter = procStatusBytes("VmRSS:");
        Long residentPeak = procStatusBytes("VmHWM:");

        String pressure = workload.equals("bounded-async")
            ? "Reactive Streams demand with publishOn prefetch " + queueItems
            : "synchronous Reactive Streams demand";
        String scheduler = workload.equals("bounded-async") ? "Schedulers.single publishOn" : "subscriber thread";
        String notes = workload.equals("bounded-async")
            ? "[\"publishOn uses the pinned prefetch as the bounded asynchronous comparison boundary.\",\"Reactor may fuse adjacent synchronous operators; the configuration is reported and not disabled.\"]"
            : "[\"Synchronous subscription and steady execution cannot be separated without changing the graph; start_ns is unavailable.\",\"Reactor operator fusion remains at the pinned implementation default.\"]";

        System.out.printf(Locale.ROOT,
            "{\"schema\":\"conduit.comparative-raw-sample\",\"schema_version\":0,\"fixture_revision\":0," +
            "\"runtime\":{\"id\":\"reactor-core\",\"comparison_role\":\"reactive-runtime\",\"version\":\"3.8.6\",\"execution_mode\":\"%s\",\"build_profile\":\"JVM default\",\"scheduler\":\"%s\",\"fusion\":\"implementation-default\",\"batching\":\"implementation-default\",\"concurrency\":1}," +
            "\"workload\":{\"id\":\"%s\",\"operators\":%d,\"input_values\":%d,\"queue_capacity_items\":%d,\"ordering\":\"source order; merge uses Reactor merge ordering\",\"pressure\":\"%s\",\"terminal\":\"complete after all requested values drain\",\"loss\":\"none\",\"slow_consumer_yields\":0,\"recovery_after_outputs\":0}," +
            "\"exact_identity\":{\"logical_fixture\":\"comparative-local-depth/%s/%d/%d/%d/%d\",\"plan_identity\":null,\"source_semantic_hash\":null,\"artifact_digest\":null}," +
            "\"sample_kind\":\"%s\",\"trial\":%d,\"thermal_state\":\"%s\",\"phases\":{\"assembly_ns\":%d,\"plan_seal_ns\":null,\"start_ns\":null,\"steady_ns\":%d,\"pressure_ns\":null,\"recovery_ns\":null}," +
            "\"process_cpu_ns\":%d,\"outcomes\":{\"offered\":%d,\"admitted\":%d,\"completed_useful\":%d,\"rejected\":0,\"sampled\":0,\"coalesced\":0,\"dropped\":0,\"retried\":0,\"terminal\":1}," +
            "\"allocations\":{\"scope\":\"unavailable-without-JVM-agent\",\"calls\":null,\"bytes\":null}," +
            "\"memory\":{\"resident_before_bytes\":%s,\"resident_after_bytes\":%s,\"resident_peak_bytes\":%s,\"planned_memory_bytes\":null,\"executor_overhead_bytes\":null,\"queue_items_high_water\":null,\"queue_payload_bytes_high_water\":null,\"ready_slots_high_water\":null,\"evidence_slots_high_water\":null}," +
            "\"latency\":{\"clock\":\"System.nanoTime monotonic\",\"sample_stride\":%d,\"samples_ns\":%s},\"semantic_notes\":%s}%n",
            workload.equals("bounded-async") ? "bounded-asynchronous" : "synchronous",
            scheduler, workload, operators, values, workload.equals("bounded-async") ? queueItems : 0,
            pressure, workload, operators, values, queueItems, stride,
            sampleKind, trial, thermalState, assemblyNs, steadyNs, cpuNs,
            values, acceptedValues.get(), usefulOutputs, nullableLong(residentBefore), nullableLong(residentAfter),
            nullableLong(residentPeak), stride, latencyJson(latencies), notes);
    }

    public static void main(String[] args) {
        int warmupTrials = Integer.parseInt(argument(args, "warmup-trials", "2"));
        int measuredTrials = Integer.parseInt(argument(args, "measured-trials", "9"));
        if (warmupTrials < 1 || measuredTrials < 1) {
            throw new IllegalArgumentException("warmup and measured trial counts must be positive");
        }
        boolean identityLoop = flag(args, "identity-loop");
        for (int trial = 0; trial < warmupTrials; trial++) {
            if (identityLoop) {
                runIdentitySample(args, "warmup", trial, trial == 0 ? "cold" : "warming");
            } else {
                runSample(args, "warmup", trial, trial == 0 ? "cold" : "warming");
            }
        }
        for (int trial = 0; trial < measuredTrials; trial++) {
            if (identityLoop) {
                runIdentitySample(args, "measured", trial, "warmed");
            } else {
                runSample(args, "measured", trial, "warmed");
            }
        }
        Schedulers.shutdownNow();
    }
}
