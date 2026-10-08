/* Local development only: pinned scalar full-float, no product linkage.
 * Input: first retained feature20 (warm), then repeated epoch u64, period i32,
 * feature20. Explicit current periods are retained Source observations.
 * Output: warm period+history128+state837, then epoch, current period,
 * condition320+history128+PCM160+state837 for each complete row.
 */
#include <stdint.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#include "fargan.c"
static int finite_features(const float *features) {
  int i;
  for(i=0;i<20;i++)if(!isfinite(features[i]))return 0;
  return 1;
}
static int state(FILE *out, const FARGANState *st) {
  return fwrite(st->fwc0_mem,4,164,out)==164 &&
    fwrite(st->gru1_state,4,160,out)==160 &&
    fwrite(st->gru2_state,4,128,out)==128 &&
    fwrite(st->gru3_state,4,128,out)==128 &&
    fwrite(st->pitch_buf,4,256,out)==256 &&
    fwrite(&st->deemph_mem,4,1,out)==1;
}
int main(int argc,char **argv) {
  FARGANState st; FILE *in,*out;
  float initial[320]={0},warm[100],features[20],condition[320],pcm[160];
  uint64_t epoch; int32_t period; int i;
  if(sizeof(float)!=4 || sizeof(int)!=4)return 9;
  { uint32_t one=1,bits;float f=1.f;memcpy(&bits,&f,4);
    if(*(unsigned char *)&one!=1 || bits!=0x3f800000U)return 9; }
  if(argc!=3)return 2;
  in=fopen(argv[1],"rb");out=fopen(argv[2],"wb");if(!in||!out)return 3;
  if(fread(features,4,20,in)!=20 || !finite_features(features))return 4;
  { double warm_period=floor(.5+256./pow(2.f,((1./60.)*((features[NB_BANDS]+1.5)*60))));
    if(!isfinite(warm_period) || warm_period<32 || warm_period>255)return 4; }
  for(i=0;i<5;i++)memcpy(warm+20*i,features,80);
  fargan_init(&st);fargan_cont(&st,initial,warm);
  if(fwrite(&st.last_period,4,1,out)!=1 || fwrite(st.cond_conv1_state,4,128,out)!=128 || !state(out,&st))return 5;
  for(;;) {
    size_t epoch_bytes=fread(&epoch,1,8,in);
    if(epoch_bytes==0 && feof(in))break;
    if(epoch_bytes!=8)return 8;
    if(fread(&period,4,1,in)!=1 || fread(features,4,20,in)!=20 || !finite_features(features) || period<32 || period>255)return 6;
    compute_fargan_cond(&st,condition,features,period);
    for(i=0;i<4;i++)run_fargan_subframe(&st,pcm+40*i,condition+80*i,st.last_period);
    st.last_period=period;
    if(fwrite(&epoch,8,1,out)!=1 || fwrite(&period,4,1,out)!=1 ||
       fwrite(condition,4,320,out)!=320 || fwrite(st.cond_conv1_state,4,128,out)!=128 ||
       fwrite(pcm,4,160,out)!=160 || !state(out,&st))return 7;
  }
  if(!feof(in))return 8;
  fclose(in);fclose(out);return 0;
}
