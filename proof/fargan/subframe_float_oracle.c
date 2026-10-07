/* Local development oracle only; no product runtime linkage. */
#include <stdio.h>
#include <string.h>
#include "fargan.c"
static void state(FILE *out, const FARGANState *st) {
  fwrite(st->fwc0_mem,4,164,out);
  fwrite(st->gru1_state,4,160,out);
  fwrite(st->gru2_state,4,128,out);
  fwrite(st->gru3_state,4,128,out);
  fwrite(st->pitch_buf,4,256,out);
  fwrite(&st->deemph_mem,4,1,out);
}
int main(int argc,char **argv) {
  FARGANState st;
  FILE *in,*out;
  float initial[320]={0},warm[100],features[20],condition[320],pcm[40];
  int i,subframe,period;
  if(argc!=3)return 2;
  in=fopen(argv[1],"rb");out=fopen(argv[2],"wb");
  if(!in||!out)return 3;
  if(fread(features,4,20,in)!=20)return 4;
  for(i=0;i<5;i++)memcpy(warm+i*20,features,80);
  fargan_init(&st);fargan_cont(&st,initial,warm);
  do {
    period=(int)floor(.5+256./pow(2.f,((1./60.)*((features[NB_BANDS]+1.5)*60))));
    compute_fargan_cond(&st,condition,features,period);
    for(subframe=0;subframe<4;subframe++) {
      fwrite(&st.last_period,4,1,out);
      fwrite(condition+80*subframe,4,80,out);
      state(out,&st);
      run_fargan_subframe(&st,pcm,condition+80*subframe,st.last_period);
      fwrite(pcm,4,40,out);state(out,&st);
    }
    st.last_period=period;
  }while(fread(features,4,20,in)==20);
  fclose(in);fclose(out);return 0;
}
