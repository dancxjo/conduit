// Local feasibility worker, not a Conduit Back or a performance admission.
// Input-major F32 projection, fixed input-order round-to-nearest operations.
#include <cuda_runtime.h>
#include <algorithm>
#include <immintrin.h>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <stdexcept>
#include <string>
#include <vector>
using Clock = std::chrono::steady_clock;
static long long ns(Clock::time_point t) { return std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now()-t).count(); }
static void check(cudaError_t s, const char* stage) {
    if (s != cudaSuccess) throw std::runtime_error(std::string(stage)+":"+cudaGetErrorName(s));
}
__global__ void project(const float* w, const float* x, float* y, int inputs, int outputs) {
    int o=blockIdx.x*blockDim.x+threadIdx.x;
    if (o<outputs) {
        float sum=0.0f;
        for(int i=0;i<inputs;i++) sum=__fadd_rn(sum,__fmul_rn(w[i*outputs+o],x[i]));
        y[o]=sum;
    }
}
static void cpu(const std::vector<float>& w, const std::vector<float>& x, std::vector<float>& y, int inputs, int outputs) {
    std::fill(y.begin(),y.end(),0.0f);
    for(int i=0;i<inputs;i++) for(int o=0;o<outputs;o++) y[o]=y[o]+w[i*outputs+o]*x[i];
}
int main(int argc,char** argv) {
    auto owner=Clock::now();
    float *dw=nullptr,*dx=nullptr,*dy=nullptr;
    bool context=false;
    try {
        if(argc!=9) throw std::runtime_error("arguments: blob offset inputs outputs repetitions device quota mode");
        size_t offset=std::stoull(argv[2]); int inputs=std::stoi(argv[3]), outputs=std::stoi(argv[4]);
        int repetitions=std::stoi(argv[5]),device=std::stoi(argv[6]); size_t quota=std::stoull(argv[7]);
        std::string mode=argv[8];
        if(inputs<1||inputs>1024||outputs<1||outputs>1024||repetitions<3||repetitions>101) throw std::runtime_error("finite-shape-or-trial-refusal");
        size_t wb=size_t(inputs)*outputs*sizeof(float), xb=size_t(inputs)*sizeof(float), yb=size_t(outputs)*sizeof(float);
        size_t required=wb+xb+yb;
        // Policy checks precede context/allocation; failed admission never falls back.
        if(required>quota || quota>16*1024*1024) throw std::runtime_error("finite-device-memory-quota-refusal");
        if(mode!="normal"&&mode!="tiny-normal"&&mode!="subnormal") throw std::runtime_error("unsupported-numeric-probe");
        std::vector<float> w(size_t(inputs)*outputs),x(inputs),reference(outputs),actual(outputs);
        std::ifstream file(argv[1],std::ios::binary); file.seekg(offset);
        if(!file.read(reinterpret_cast<char*>(w.data()),wb)) throw std::runtime_error("exact-weight-read-refusal");
        for(float f:w) if(!std::isfinite(f)) throw std::runtime_error("nonfinite-weight-refusal");
        for(int i=0;i<inputs;i++) x[i]=mode=="subnormal" ? std::ldexp(float(i%9-4),-140) :
            std::ldexp(float((i*17)%1024-512),mode=="tiny-normal"?-120:-10);
        _mm_setcsr(_mm_getcsr()&~0x8040u); // Explicit CPU FTZ/DAZ off.
        auto stage=Clock::now(); check(cudaSetDevice(device),"set-device");
        check(cudaFree(nullptr),"create-context"); context=true;
        long long context_ns=ns(stage);
        cudaDeviceProp prop; check(cudaGetDeviceProperties(&prop,device),"properties");
        size_t available,total;check(cudaMemGetInfo(&available,&total),"memory-status");
        if(required>available) throw std::runtime_error("observed-device-memory-unavailable");
        stage=Clock::now();check(cudaMalloc(&dw,wb),"allocate-weights");check(cudaMalloc(&dx,xb),"allocate-input");check(cudaMalloc(&dy,yb),"allocate-output");
        long long allocation_ns=ns(stage);
        stage=Clock::now();check(cudaMemcpy(dw,w.data(),wb,cudaMemcpyHostToDevice),"upload-weights");long long weight_upload_ns=ns(stage);
        int driver,runtime;check(cudaDriverGetVersion(&driver),"driver-version");check(cudaRuntimeGetVersion(&runtime),"runtime-version");
        printf("{\"record\":\"admission\",\"device\":\"%s\",\"ordinal\":%d,\"compute\":[%d,%d],\"driver\":%d,\"runtime\":%d,\"device_total_bytes\":%zu,\"device_available_bytes\":%zu,\"policy_quota_bytes\":%zu,\"admitted_device_bytes\":%zu,\"device_allocations\":3,\"context_ns\":%lld,\"allocation_ns\":%lld,\"weights_upload_ns\":%lld,\"weights_bytes\":%zu,\"inputs\":%d,\"outputs\":%d,\"mode\":\"%s\"}\n",prop.name,device,prop.major,prop.minor,driver,runtime,total,available,quota,required,context_ns,allocation_ns,weight_upload_ns,wb,inputs,outputs,mode.c_str());
        for(int trial=0;trial<repetitions;trial++) {
            // Every trial includes a fresh CPU reference and input/output copies.
            stage=Clock::now();cpu(w,x,reference,inputs,outputs);long long cpu_ns=ns(stage);
            for(float f:reference) if(!std::isfinite(f)) throw std::runtime_error("nonfinite-result-refusal");
            auto full=Clock::now();stage=Clock::now();check(cudaMemcpy(dx,x.data(),xb,cudaMemcpyHostToDevice),"upload-input");long long input_ns=ns(stage);
            stage=Clock::now();project<<<(outputs+127)/128,128>>>(dw,dx,dy,inputs,outputs);check(cudaGetLastError(),"launch");long long launch_ns=ns(stage);
            stage=Clock::now();check(cudaDeviceSynchronize(),"synchronize");long long synchronize_ns=ns(stage);
            stage=Clock::now();check(cudaMemcpy(actual.data(),dy,yb,cudaMemcpyDeviceToHost),"download-output");long long download_ns=ns(stage);long long gpu_ns=ns(full);
            stage=Clock::now();bool equal=std::memcmp(reference.data(),actual.data(),yb)==0;long long acceptance_ns=ns(stage);
            printf("{\"record\":\"trial\",\"index\":%d,\"cpu_ns\":%lld,\"input_upload_ns\":%lld,\"launch_submission_ns\":%lld,\"synchronization_ns\":%lld,\"output_download_ns\":%lld,\"gpu_complete_ns\":%lld,\"exact_acceptance_ns\":%lld,\"input_bytes\":%zu,\"output_bytes\":%zu,\"equal_bits\":%s}\n",trial,cpu_ns,input_ns,launch_ns,synchronize_ns,download_ns,gpu_ns,acceptance_ns,xb,yb,equal?"true":"false");
            if(!equal) throw std::runtime_error("numeric-law-bitwise-refusal");
        }
        stage=Clock::now();check(cudaFree(dy),"free-output");dy=nullptr;check(cudaFree(dx),"free-input");dx=nullptr;check(cudaFree(dw),"free-weights");dw=nullptr;
        check(cudaDeviceReset(),"destroy-owned-context");context=false;
        file.close();
        std::vector<float>().swap(w);std::vector<float>().swap(x);std::vector<float>().swap(reference);std::vector<float>().swap(actual);
        printf("{\"record\":\"cleanup\",\"success\":true,\"cleanup_ns\":%lld,\"complete_owner_ns\":%lld,\"conduit_gpu_offer_admitted\":false}\n",ns(stage),ns(owner));
        return 0;
    } catch(const std::exception& e) {
        // Best-effort cleanup is reported separately; a refusal never runs CPU fallback.
        cudaError_t a=dy?cudaFree(dy):cudaSuccess,b=dx?cudaFree(dx):cudaSuccess,c=dw?cudaFree(dw):cudaSuccess;
        cudaError_t d=context?cudaDeviceReset():cudaSuccess;
        printf("{\"record\":\"refusal\",\"reason\":\"%s\",\"cleanup_success\":%s,\"complete_owner_ns\":%lld,\"conduit_gpu_offer_admitted\":false}\n",e.what(),(a==cudaSuccess&&b==cudaSuccess&&c==cudaSuccess&&d==cudaSuccess)?"true":"false",ns(owner));
        return 2;
    }
}
