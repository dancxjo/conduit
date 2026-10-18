/* Independent scalar development oracle; never linked into product Play. */
#include <stdio.h>
#include <string.h>
#include "fargan.c"
static void state(FILE *out, const FARGANState *st) {
  fwrite(st->fwc0_mem,4,164,out); fwrite(st->gru1_state,4,160,out);
  fwrite(st->gru2_state,4,128,out); fwrite(st->gru3_state,4,128,out);
  fwrite(st->pitch_buf,4,256,out); fwrite(&st->deemph_mem,4,1,out);
  fwrite(st->cond_conv1_state,4,128,out); fwrite(&st->last_period,4,1,out);
}
int main(int argc,char **argv) {
  FARGANState st; FILE *in,*out;
  float initial[320]={0},warm[100],features[20],pcm[160]; int i;
  if(argc!=3)return 2;
  in=fopen(argv[1],"rb");out=fopen(argv[2],"wb");if(!in||!out)return 3;
  if(fread(features,4,20,in)!=20)return 4;
  for(i=0;i<5;i++)memcpy(warm+i*20,features,80);
  fargan_init(&st); fargan_cont(&st,initial,warm); state(out,&st);
  do { fwrite(features,4,20,out); fargan_synthesize(&st,pcm,features);
       fwrite(pcm,4,160,out); state(out,&st);
  }while(fread(features,4,20,in)==20);
  fclose(in);fclose(out);return 0;
}
