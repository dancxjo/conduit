/* Independent private warm-only reference; never linked into product Play. */
#include <stdio.h>
#include "fargan.c"
static int state(FILE *out, const FARGANState *st) {
  return fwrite(st->fwc0_mem,4,164,out)==164
    && fwrite(st->gru1_state,4,160,out)==160
    && fwrite(st->gru2_state,4,128,out)==128
    && fwrite(st->gru3_state,4,128,out)==128
    && fwrite(st->pitch_buf,4,256,out)==256
    && fwrite(&st->deemph_mem,4,1,out)==1
    && fwrite(st->cond_conv1_state,4,128,out)==128
    && fwrite(&st->last_period,4,1,out)==1;
}
int main(int argc, char **argv) {
  FARGANState st;
  float context[320], features[100];
  FILE *in,*out;
  int ok;
  if(argc!=3)return 2;
  in=fopen(argv[1],"rb");if(!in)return 3;
  if(fread(context,4,320,in)!=320 || fread(features,4,100,in)!=100
     || fgetc(in)!=EOF){fclose(in);return 4;}
  fclose(in);
  fargan_init(&st);
  fargan_cont(&st,context,features);
  out=fopen(argv[2],"wb");if(!out)return 5;
  ok=state(out,&st);
  if(fclose(out)!=0)ok=0;
  return ok?0:6;
}
